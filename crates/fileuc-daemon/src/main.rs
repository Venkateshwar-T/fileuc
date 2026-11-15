#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fileuc_daemon::autostart::set_auto_start;
use fileuc_daemon::dispatcher::{dispatch, ConversionRequest, ConversionResult};
use fileuc_daemon::ipc::{send_message, start_server, IpcMessage};
use fileuc_daemon::single_instance::acquire;
use fileuc_hook::gesture::{GestureDetector, GestureEvent, Modifiers};
use fileuc_hook::mouse_hook::{
    install, set_key_capture, set_mouse_capture_rect, set_qr_shortcut, HookEvent,
};
use fileuc_hook::shortcut::parse_shortcut;
use fileuc_daemon::share_server::{start_share_server, ShareServerEvent, ShareServerHandle};
use fileuc_hud::donut_geometry::HitResult;
use fileuc_hud::drop_target::DropTargetEvent;
use fileuc_hud::bulk_rename_card::{render_bulk_rename_card, BulkRenameCardState};
use fileuc_hud::font::FontContext;
use fileuc_hud::hud_window::HudWindow;
use fileuc_hud::password_card::{render_password_card, PasswordCardMode, PasswordCardState};
use fileuc_hud::progress_ring::ProgressTracker;
use fileuc_hud::qr_card::{render_qr_card, QrCardState, QrShareTab};
use fileuc_hud::quick_qr_card::{render_quick_qr_card, QuickQrCardState, QuickQrTab};
use fileuc_hud::reduce_size_card::{render_reduce_size_card, ReduceSizeCardState};
use fileuc_hud::renderer::{render_donut, render_progress, DonutState};
use fileuc_hud::smart_filter::filter_actions;
use fileuc_hud::theme::Theme;
use fileuc_settings::store::SettingsStore;
use fileuc_types::ConversionAction;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
};

struct PopupDragController {
    start_cursor_x: i32,
    start_cursor_y: i32,
    start_win_x: i32,
    start_win_y: i32,
}

fn get_missing_plugins() -> Vec<ConversionAction> {
    let mut missing = Vec::new();
    if fileuc_convert::doc::find_pdf2docx().is_none() {
        missing.push(ConversionAction::OfficeGroup);
        missing.push(ConversionAction::PdfToDocx);
        missing.push(ConversionAction::ExcelToPdf);
        missing.push(ConversionAction::PdfToExcel);
        missing.push(ConversionAction::PdfOcr);
    }
    if fileuc_convert::media::find_ffmpeg().is_none() {
        missing.push(ConversionAction::VideoToGif);
        missing.push(ConversionAction::VideoToMp3);
        missing.push(ConversionAction::CompressVideo);
    }
    missing
}

use fileuc_daemon::clipboard::get_clipboard_text;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        let log_dir = std::path::PathBuf::from(app_data).join("FileUC");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("panic.log");
        let msg = format!("PANIC at {:?}:\n{}\n\n", std::time::SystemTime::now(), info);
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&log_path).and_then(|mut f| {
            use std::io::Write;
            f.write_all(msg.as_bytes())
        });
        eprintln!("{}", msg);
    }));

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = OleInitialize(None);
    }

    let mutex_name = "FileUC_SingleInstance_Mutex";
    let pipe_name = "FileUC_Ipc_Pipe";

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--settings" || a == "settings") {
        let tab = args
            .windows(2)
            .find(|w| w[0] == "--tab")
            .and_then(|w| w[1].parse::<i32>().ok())
            .unwrap_or(0);
        fileuc_settings::run_tauri(tab);
        return;
    }

    if args.iter().any(|a| a == "--quit" || a == "quit" || a == "-q" || a == "--exit" || a == "exit") {
        let _ = send_message(pipe_name, IpcMessage::Quit);
        return;
    }

    let is_silent = args.iter().any(|a| a == "--silent" || a == "--autostart" || a == "-s");

    let _guard = match acquire(mutex_name) {
        Ok(guard) => Some(guard),
        Err(_) => {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
                let _ = AllowSetForegroundWindow(0xFFFFFFFF);
            }
            let _ = send_message(pipe_name, IpcMessage::OpenSettings);
            return;
        }
    };

    // Pre-warm ffmpeg probe on background thread so find_ffmpeg() is instantly cached in OnceLock
    std::thread::Builder::new()
        .name("ffmpeg-warmup".into())
        .spawn(|| {
            let _ = fileuc_convert::media::find_ffmpeg();
        })
        .ok();

    // Pre-warm COM ShellWindows connection so first drag detection is instant (<1ms)
    let _ = fileuc_daemon::explorer::get_selected_files();

    // Load persistent settings
    let settings_path = SettingsStore::default_path();
    let settings_store = Arc::new(SettingsStore::new(settings_path));
    let current_settings = Arc::new(Mutex::new(settings_store.load()));

    // Apply auto-start
    {
        let st = current_settings.lock().unwrap();
        let _ = set_auto_start(st.auto_start);
    }
    fileuc_daemon::context_menu::cleanup_context_menu();

    let should_quit = Arc::new(AtomicBool::new(false));
    let settings_changed = Arc::new(AtomicBool::new(false));

    // Initialize SettingsController
    let settings_store_cb = settings_store.clone();
    let current_settings_cb = current_settings.clone();
    let settings_changed_cb = settings_changed.clone();
    let settings_controller = Arc::new(fileuc_settings::SettingsController::spawn(
        move |updated| {
            let _ = settings_store_cb.save(&updated);
            let _ = set_auto_start(updated.auto_start);
            *current_settings_cb.lock().unwrap() = updated;
            settings_changed_cb.store(true, Ordering::Release);
        },
        {
            let current_settings_toggle = current_settings.clone();
            let settings_store_toggle = settings_store.clone();
            let settings_changed_toggle = settings_changed.clone();
            move |enabled| {
                let mut st = current_settings_toggle.lock().unwrap();
                st.service_enabled = enabled;
                let _ = settings_store_toggle.save(&st);
                settings_changed_toggle.store(true, Ordering::Release);
            }
        },
    ));

    // Start named pipe IPC server
    let ctrl_for_ipc = settings_controller.clone();
    let settings_for_ipc = current_settings.clone();
    let settings_store_ipc = settings_store.clone();
    let settings_changed_ipc = settings_changed.clone();
    let quit_flag = should_quit.clone();
    let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    let _server = start_server(pipe_name, move |msg| match msg {
        IpcMessage::OpenSettings => {
            let st = settings_for_ipc.lock().unwrap().clone();
            ctrl_for_ipc.show(st);
        }
        IpcMessage::OpenSettingsTab(tab) => {
            let st = settings_for_ipc.lock().unwrap().clone();
            ctrl_for_ipc.show_tab(st, tab);
        }
        IpcMessage::ReloadSettings => {
            let loaded = settings_store_ipc.load();
            *settings_for_ipc.lock().unwrap() = loaded.clone();
            let _ = set_auto_start(loaded.auto_start);
            settings_changed_ipc.store(true, Ordering::Release);
        }
        IpcMessage::ToggleService(enabled) => {
            let mut st = settings_for_ipc.lock().unwrap();
            st.service_enabled = enabled;
            let _ = settings_store_ipc.save(&st);
            settings_changed_ipc.store(true, Ordering::Release);
        }
        IpcMessage::Quit => {
            quit_flag.store(true, Ordering::SeqCst);
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_NULL};
                use windows::Win32::Foundation::{WPARAM, LPARAM};
                let _ = PostThreadMessageW(main_thread_id, WM_NULL, WPARAM(0), LPARAM(0));
            }
        }
    })
    .expect("Failed to start IPC server");

    if !is_silent {
        let st = current_settings.lock().unwrap().clone();
        settings_controller.show(st);
    }

    // Create Mouse Hook
    let (hook_tx, hook_rx) = mpsc::channel();
    let _hook = install(hook_tx).expect("Failed to install mouse hook");
    fileuc_hook::mouse_hook::set_target_thread_id(main_thread_id);

    // Create Unified HUD Window & Drop Target
    let (drop_tx, drop_rx) = mpsc::channel();
    let mut hud = HudWindow::new(drop_tx).expect("Failed to create HudWindow");

    // Create System Tray Event Channel & Icon (if enabled in settings)
    let (tray_tx, tray_rx) = mpsc::channel();
    let initial_service_enabled = current_settings.lock().unwrap_or_else(|e| e.into_inner()).service_enabled;
    let mut tray: Option<fileuc_daemon::tray::TrayIcon> = if current_settings.lock().unwrap_or_else(|e| e.into_inner()).show_tray_icon {
        Some(fileuc_daemon::tray::TrayIcon::new(tray_tx.clone(), initial_service_enabled))
    } else {
        None
    };

    // State machine
    let initial_mod = {
        let st = current_settings.lock().unwrap_or_else(|e| e.into_inner());
        match st.trigger_modifier.as_str() {
            "Alt" => Modifiers::ALT,
            "Shift" => Modifiers::SHIFT,
            _ => Modifiers::CTRL,
        }
    };
    let mut cached_service_enabled = initial_service_enabled;
    let mut cached_theme_choice = current_settings.lock().unwrap_or_else(|e| e.into_inner()).theme;
    let mut cached_theme = Theme::resolve(match cached_theme_choice {
        fileuc_settings::store::ThemeChoice::Dark => "dark",
        fileuc_settings::store::ThemeChoice::Light => "light",
        fileuc_settings::store::ThemeChoice::System => "system",
    });

    let mut detector = GestureDetector::new(5, initial_mod);
    let mut donut_state: Option<DonutState> = None;
    let mut current_actions: Vec<ConversionAction> = Vec::new();
    let mut current_files: Vec<PathBuf> = Vec::new();
    let mut progress_tracker: Option<ProgressTracker> = None;
    let mut qr_card_state: Option<QrCardState> = None;
    let mut quick_qr_card_state: Option<QuickQrCardState> = None;
    let mut password_card_state: Option<PasswordCardState> = None;
    let mut reduce_size_card_state: Option<ReduceSizeCardState> = None;
    let mut bulk_rename_card_state: Option<BulkRenameCardState> = None;
    let mut pinggy_tunnel: Option<fileuc_daemon::pinggy::PinggyTunnel> = None;
    let mut cloudflare_tunnel: Option<fileuc_daemon::cloudflared::CloudflareTunnel> = None;
    let mut cloudflared_dl_rx: Option<mpsc::Receiver<fileuc_daemon::cloudflared::DownloadProgress>> = None;
    let mut share_server: Option<ShareServerHandle> = None;
    let mut conversion_in_flight: Option<(JoinHandle<ConversionResult>, Arc<AtomicU32>)> = None;
    let mut hud_center = hud.hud_center();
    let mut pending_drag_ended: Option<Instant> = None;
    let mut popup_drag: Option<PopupDragController> = None;
    let _app_icon = unsafe {
        use windows::Win32::Foundation::HINSTANCE;
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        fileuc_daemon::tray::load_app_icon(hinstance)
    };
    let mut order_tracker = fileuc_daemon::order_tracker::SelectionOrderTracker::new();
    let detected_wifi_ssid: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    {
        let wifi_clone = detected_wifi_ssid.clone();
        std::thread::spawn(move || {
            if let Some(ssid) = fileuc_daemon::wifi::detect_wifi_ssid() {
                *wifi_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(ssid);
            }
        });
    }

    // Register initial quick QR shortcut from persistent settings
    {
        let st = current_settings.lock().unwrap_or_else(|e| e.into_inner());
        set_qr_shortcut(parse_shortcut(&st.qr_shortcut));
    }

    let fonts = FontContext::new();
    let mut pixmap = tiny_skia::Pixmap::new(hud.size(), hud.size()).unwrap();
    let mut cursor_is_hand = false;

    // Warm up DWM layered window context, tiny-skia path rendering, and font glyph rasterizer
    hud.warmup_surface();
    {
        let warmup_actions = vec![
            ConversionAction::ToPng,
            ConversionAction::ToJpg,
            ConversionAction::ToPdf,
            ConversionAction::CreateZip,
            ConversionAction::LockPdf,
            ConversionAction::UnlockPdf,
            ConversionAction::Share,
        ];
        let mut warmup_state = DonutState::new(warmup_actions, 1, hud.dpi_scale());
        warmup_state.set_file_label("1 FILE");
        render_donut(&mut warmup_state, &Theme::dark(), &fonts, &mut pixmap);
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
    }

    // Trim initial startup heap & module working set so idle footprint is minimal (<8MB)
    fileuc_daemon::trim_working_set();

    let mut was_active = false;

    while !should_quit.load(Ordering::SeqCst) {
        let mut donut_dirty = false;
        let progress_dirty = false;
        let mut password_card_dirty = false;
        let mut qr_card_dirty = false;
        let mut quick_qr_card_dirty = false;
        let mut reduce_size_card_dirty = false;
        let mut bulk_rename_card_dirty = false;
        let mut submit_password = false;
        let mut submit_reduce_size = false;
        let mut submit_bulk_rename = false;

        // Sync trigger modifier, theme, tray, quick QR shortcut and service state dynamically only when changed
        if settings_changed.swap(false, Ordering::Acquire) {
            let st = current_settings.lock().unwrap().clone();
            let new_mod = match st.trigger_modifier.as_str() {
                "Alt" => Modifiers::ALT,
                "Shift" => Modifiers::SHIFT,
                _ => Modifiers::CTRL,
            };
            detector.set_required_modifier(new_mod);
            set_qr_shortcut(parse_shortcut(&st.qr_shortcut));
            cached_service_enabled = st.service_enabled;
            cached_theme_choice = st.theme;
            cached_theme = Theme::resolve(match cached_theme_choice {
                fileuc_settings::store::ThemeChoice::Dark => "dark",
                fileuc_settings::store::ThemeChoice::Light => "light",
                fileuc_settings::store::ThemeChoice::System => "system",
            });

            if st.show_tray_icon && tray.is_none() {
                tray = Some(fileuc_daemon::tray::TrayIcon::new(tray_tx.clone(), cached_service_enabled));
            } else if !st.show_tray_icon && tray.is_some() {
                tray = None;
            }
            if let Some(ref mut t) = tray {
                t.set_service_enabled(cached_service_enabled);
            }
        }

        let service_enabled = cached_service_enabled;

        // If service was paused while a gesture or HUD was open, cancel it immediately
        if !service_enabled && (donut_state.is_some() || quick_qr_card_state.is_some()) {
            donut_state = None;
            dismiss_quick_qr_card(&mut quick_qr_card_state, &mut hud);
            hud.hide();
            current_files.clear();
            current_actions.clear();
        }

        // Process Tray Events from dedicated Tray Window
        while let Ok(tray_event) = tray_rx.try_recv() {
            match tray_event {
                fileuc_daemon::tray::TrayEvent::OpenSettings => {
                    let st = current_settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    settings_controller.show(st);
                }
                fileuc_daemon::tray::TrayEvent::ToggleService => {
                    let new_val = {
                        let mut st = current_settings.lock().unwrap_or_else(|e| e.into_inner());
                        st.service_enabled = !st.service_enabled;
                        let _ = settings_store.save(&st);
                        st.service_enabled
                    };
                    settings_changed.store(true, Ordering::Release);
                    settings_controller.set_service_enabled(new_val);
                    if let Some(ref mut t) = tray {
                        t.set_service_enabled(new_val);
                    }
                }
                fileuc_daemon::tray::TrayEvent::Exit => {
                    settings_controller.quit();
                    should_quit.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }

        // 1. Process Windows Messages
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    should_quit.store(true, Ordering::SeqCst);
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 2. Process Hook Events (with high-frequency MouseMove coalescing)
        let mut hook_events = Vec::new();
        while let Ok(event) = hook_rx.try_recv() {
            match event {
                HookEvent::MouseMove { .. } => {
                    if let Some(HookEvent::MouseMove { .. }) = hook_events.last() {
                        hook_events.pop();
                    }
                    hook_events.push(event);
                }
                other => {
                    hook_events.push(other);
                }
            }
        }

        for event in hook_events {
            let gesture = match event {
                HookEvent::MouseDown { btn, x, y, mods } => {
                    if let Some(ref mut card) = quick_qr_card_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let cx = hud.size() as f32 / 2.0;
                        let cy = hud.size() as f32 / 2.0;
                        if card.hit_test_close(rel_x, rel_y, cx, cy) {
                            dismiss_quick_qr_card(&mut quick_qr_card_state, &mut hud);
                        } else if let Some(tab) = card.hit_test_tab(rel_x, rel_y, cx, cy) {
                            card.switch_tab(tab);
                            if tab == QuickQrTab::Wifi && card.detected_ssid.is_none() {
                                let wifi_clone = detected_wifi_ssid.clone();
                                std::thread::spawn(move || {
                                    if let Some(ssid) = fileuc_daemon::wifi::detect_wifi_ssid() {
                                        *wifi_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(ssid);
                                    }
                                });
                            }
                            quick_qr_card_dirty = true;
                        } else if card.hit_test_save(rel_x, rel_y, cx, cy) {
                            if let Some(ref matrix) = card.qr_matrix {
                                let prefix = match card.tab {
                                    QuickQrTab::Link => "Link",
                                    QuickQrTab::Text => "Text",
                                    QuickQrTab::Wifi => "WiFi",
                                };
                                if let Some(_path) = fileuc_daemon::save_qr_to_desktop(matrix, prefix) {
                                    card.saved_feedback_timer = Some(Instant::now());
                                    quick_qr_card_dirty = true;
                                }
                            }
                        } else if card.hit_test_card(rel_x, rel_y, cx, cy) {
                            popup_drag = Some(PopupDragController {
                                start_cursor_x: x,
                                start_cursor_y: y,
                                start_win_x: hud.screen_x(),
                                start_win_y: hud.screen_y(),
                            });
                        }
                    } else if let Some(ref mut card) = password_card_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let cx = hud.size() as f32 / 2.0;
                        let cy = hud.size() as f32 / 2.0;
                        if card.hit_test_close(rel_x, rel_y, cx, cy) {
                            dismiss_password_card(
                                &mut password_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if card.hit_test_submit(rel_x, rel_y, cx, cy) {
                            submit_password = true;
                        } else if card.hit_test_card(rel_x, rel_y, cx, cy) {
                            popup_drag = Some(PopupDragController {
                                start_cursor_x: x,
                                start_cursor_y: y,
                                start_win_x: hud.screen_x(),
                                start_win_y: hud.screen_y(),
                            });
                        }
                    } else if let Some(ref mut card) = reduce_size_card_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let cx = hud.size() as f32 / 2.0;
                        let cy = hud.size() as f32 / 2.0;
                        if card.hit_test_close(rel_x, rel_y, cx, cy) {
                            dismiss_reduce_size_card(
                                &mut reduce_size_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if let Some(unit) = card.hit_test_unit(rel_x, rel_y, cx, cy) {
                            card.set_unit(unit);
                            reduce_size_card_dirty = true;
                        } else if card.hit_test_submit(rel_x, rel_y, cx, cy) {
                            submit_reduce_size = true;
                        } else if card.hit_test_card(rel_x, rel_y, cx, cy) {
                            popup_drag = Some(PopupDragController {
                                start_cursor_x: x,
                                start_cursor_y: y,
                                start_win_x: hud.screen_x(),
                                start_win_y: hud.screen_y(),
                            });
                        }
                    } else if let Some(ref mut card) = bulk_rename_card_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let cx = hud.size() as f32 / 2.0;
                        let cy = hud.size() as f32 / 2.0;
                        if card.hit_test_close(rel_x, rel_y, cx, cy) {
                            dismiss_bulk_rename_card(
                                &mut bulk_rename_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if card.hit_test_submit(rel_x, rel_y, cx, cy) {
                            submit_bulk_rename = true;
                        } else if card.hit_test_card(rel_x, rel_y, cx, cy) {
                            popup_drag = Some(PopupDragController {
                                start_cursor_x: x,
                                start_cursor_y: y,
                                start_win_x: hud.screen_x(),
                                start_win_y: hud.screen_y(),
                            });
                        }
                    } else if let Some(ref mut card) = qr_card_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let cx = hud.size() as f32 / 2.0;
                        let cy = hud.size() as f32 / 2.0;
                        if card.hit_test_close(rel_x, rel_y, cx, cy) {
                            dismiss_qr_card(
                                &mut qr_card_state,
                                &mut share_server,
                                &mut pinggy_tunnel,
                                &mut cloudflare_tunnel,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if card.hit_test_copy_btn(rel_x, rel_y, cx, cy) {
                            if let Some(url) = card.current_share_url() {
                                fileuc_daemon::clipboard::set_clipboard_text(url);
                                card.copy_feedback_timer = Some(Instant::now());
                                qr_card_dirty = true;
                            }
                        } else if card.tab == QrShareTab::Internet && !card.is_cloudflared_installed && !card.is_downloading && card.hit_test_download_btn(rel_x, rel_y, cx, cy) {
                            card.is_downloading = true;
                            card.download_pct = Some(0);
                            let (tx, rx) = mpsc::channel();
                            cloudflared_dl_rx = Some(rx);
                            fileuc_daemon::cloudflared::start_download(tx);
                            qr_card_dirty = true;
                        } else if let Some(tab) = card.hit_test_tab(rel_x, rel_y, cx, cy) {
                            if tab != card.tab {
                                match tab {
                                    QrShareTab::Local => {
                                        if let Some(mut c) = cloudflare_tunnel.take() {
                                            c.stop();
                                        }
                                        if let Some(mut p) = pinggy_tunnel.take() {
                                            p.stop();
                                        }
                                        card.switch_tab(QrShareTab::Local);
                                        qr_card_dirty = true;
                                    }
                                    QrShareTab::Internet => {
                                        card.switch_tab(QrShareTab::Internet);
                                        if card.is_cloudflared_installed && card.internet_url.is_none() {
                                            if let Some(ref s) = share_server {
                                                let port = s.port;
                                                let path = s.url.split('/').skip(3).collect::<Vec<&str>>().join("/");
                                                match fileuc_daemon::cloudflared::CloudflareTunnel::start(port, &format!("/{}", path)) {
                                                    Ok(tunnel) => {
                                                        cloudflare_tunnel = Some(tunnel);
                                                    }
                                                    Err(e) => {
                                                        card.set_tunnel_error(&e);
                                                    }
                                                }
                                            }
                                        }
                                        qr_card_dirty = true;
                                    }
                                }
                            }
                        } else if card.hit_test_card(rel_x, rel_y, cx, cy) {
                            popup_drag = Some(PopupDragController {
                                start_cursor_x: x,
                                start_cursor_y: y,
                                start_win_x: hud.screen_x(),
                                start_win_y: hud.screen_y(),
                            });
                        }
                    } else if progress_tracker.is_some() {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        if fileuc_hud::renderer::hit_test_progress(rel_x, rel_y, hud.dpi_scale()) {
                            popup_drag = Some(PopupDragController {
                                start_cursor_x: x,
                                start_cursor_y: y,
                                start_win_x: hud.screen_x(),
                                start_win_y: hud.screen_y(),
                            });
                        }
                    }
                    if popup_drag.is_none() {
                        detector.mouse_down(btn, x, y, mods)
                    } else {
                        None
                    }
                }
                HookEvent::MouseMove { x, y, mods } => {
                    let g = if popup_drag.is_none() {
                        detector.mouse_move(x, y, mods)
                    } else {
                        None
                    };

                    if let Some(ref drag) = popup_drag {
                        let dx = x - drag.start_cursor_x;
                        let dy = y - drag.start_cursor_y;
                        let new_x = drag.start_win_x + dx;
                        let new_y = drag.start_win_y + dy;
                        hud_center = hud.move_to(new_x, new_y);

                        let cx_f = hud.size() as f32 / 2.0;
                        let cy_f = hud.size() as f32 / 2.0;
                        let win_x = hud.screen_x();
                        let win_y = hud.screen_y();
                        if let Some(ref card) = quick_qr_card_state {
                            let (l, t, w, h) = card.card_bounds(cx_f, cy_f);
                            set_mouse_capture_rect(Some((
                                win_x + l as i32,
                                win_y + t as i32,
                                win_x + (l + w) as i32,
                                win_y + (t + h) as i32,
                            )));
                        } else if let Some(ref card) = password_card_state {
                            let (l, t, w, h) = card.card_bounds(cx_f, cy_f);
                            set_mouse_capture_rect(Some((
                                win_x + l as i32,
                                win_y + t as i32,
                                win_x + (l + w) as i32,
                                win_y + (t + h) as i32,
                            )));
                        } else if let Some(ref card) = reduce_size_card_state {
                            let (l, t, w, h) = card.card_bounds(cx_f, cy_f);
                            set_mouse_capture_rect(Some((
                                win_x + l as i32,
                                win_y + t as i32,
                                win_x + (l + w) as i32,
                                win_y + (t + h) as i32,
                            )));
                        } else if let Some(ref card) = bulk_rename_card_state {
                            let (l, t, w, h) = card.card_bounds(cx_f, cy_f);
                            set_mouse_capture_rect(Some((
                                win_x + l as i32,
                                win_y + t as i32,
                                win_x + (l + w) as i32,
                                win_y + (t + h) as i32,
                            )));
                        } else if let Some(ref card) = qr_card_state {
                            let (l, t, w, h) = card.card_bounds(cx_f, cy_f);
                            set_mouse_capture_rect(Some((
                                win_x + l as i32,
                                win_y + t as i32,
                                win_x + (l + w) as i32,
                                win_y + (t + h) as i32,
                            )));
                        } else if progress_tracker.is_some() {
                            let half = (44.0 * hud.dpi_scale()) as i32;
                            set_mouse_capture_rect(Some((
                                hud_center.0 - half,
                                hud_center.1 - half,
                                hud_center.0 + half,
                                hud_center.1 + half,
                            )));
                        }
                    }

                    let rel_x = (x - hud_center.0) as f32;
                    let rel_y = (y - hud_center.1) as f32;
                    let cx = hud.size() as f32 / 2.0;
                    let cy = hud.size() as f32 / 2.0;
                    let mut is_hand = false;

                    if let Some(ref mut card) = quick_qr_card_state {
                        let close_hov = card.hit_test_close(rel_x, rel_y, cx, cy);
                        let tab_hov = card.hit_test_tab(rel_x, rel_y, cx, cy);
                        let save_hov = card.hit_test_save(rel_x, rel_y, cx, cy);
                        if card.is_close_hovered != close_hov || card.hovered_tab != tab_hov || card.is_save_hovered != save_hov {
                            card.is_close_hovered = close_hov;
                            card.hovered_tab = tab_hov;
                            card.is_save_hovered = save_hov;
                            quick_qr_card_dirty = true;
                        }
                        if close_hov || tab_hov.is_some() || (save_hov && card.qr_matrix.is_some()) {
                            is_hand = true;
                        }
                    } else if let Some(ref mut card) = password_card_state {
                        let close_hov = card.hit_test_close(rel_x, rel_y, cx, cy);
                        let submit_hov = card.hit_test_submit(rel_x, rel_y, cx, cy);
                        if card.is_close_hovered != close_hov || card.is_submit_hovered != submit_hov {
                            card.is_close_hovered = close_hov;
                            card.is_submit_hovered = submit_hov;
                            password_card_dirty = true;
                        }
                        if close_hov || submit_hov {
                            is_hand = true;
                        }
                    } else if let Some(ref mut card) = reduce_size_card_state {
                        let close_hov = card.hit_test_close(rel_x, rel_y, cx, cy);
                        let unit_hov = card.hit_test_unit(rel_x, rel_y, cx, cy);
                        let submit_hov = card.hit_test_submit(rel_x, rel_y, cx, cy);
                        if card.is_close_hovered != close_hov || card.hovered_unit != unit_hov || card.is_submit_hovered != submit_hov {
                            card.is_close_hovered = close_hov;
                            card.hovered_unit = unit_hov;
                            card.is_submit_hovered = submit_hov;
                            reduce_size_card_dirty = true;
                        }
                        if close_hov || unit_hov.is_some() || submit_hov {
                            is_hand = true;
                        }
                    } else if let Some(ref mut card) = bulk_rename_card_state {
                        let close_hov = card.hit_test_close(rel_x, rel_y, cx, cy);
                        let submit_hov = card.hit_test_submit(rel_x, rel_y, cx, cy);
                        if card.is_close_hovered != close_hov || card.is_submit_hovered != submit_hov {
                            card.is_close_hovered = close_hov;
                            card.is_submit_hovered = submit_hov;
                            bulk_rename_card_dirty = true;
                        }
                        if close_hov || submit_hov {
                            is_hand = true;
                        }
                    } else if let Some(ref mut card) = qr_card_state {
                        let close_hov = card.hit_test_close(rel_x, rel_y, cx, cy);
                        let tab_hov = card.hit_test_tab(rel_x, rel_y, cx, cy);
                        let copy_hov = card.hit_test_copy_btn(rel_x, rel_y, cx, cy);
                        let dl_hov = card.hit_test_download_btn(rel_x, rel_y, cx, cy);
                        if card.is_close_hovered != close_hov
                            || card.hovered_tab != tab_hov
                            || card.is_copy_hovered != copy_hov
                            || card.is_download_btn_hovered != dl_hov
                        {
                            card.is_close_hovered = close_hov;
                            card.hovered_tab = tab_hov;
                            card.is_copy_hovered = copy_hov;
                            card.is_download_btn_hovered = dl_hov;
                            qr_card_dirty = true;
                        }
                        if close_hov || tab_hov.is_some() || copy_hov || (card.tab == QrShareTab::Internet && !card.is_cloudflared_installed && dl_hov) {
                            is_hand = true;
                        }
                    } else if donut_state.is_some() && progress_tracker.is_none() {
                        let dpi_scale = hud.dpi_scale();
                        let pop_out = 7.0 * dpi_scale;
                        if let Some(ref mut state) = donut_state {
                            let old_hovered = state.hovered_slice;
                            let old_hot = state.center_hot;
                            let old_sub = state.sub_hover;
                            let old_expanded = state.expanded_group;
                            let _ = state.track_pointer(rel_x, rel_y, pop_out);
                            if state.hovered_slice != old_hovered
                                || state.center_hot != old_hot
                                || state.sub_hover != old_sub
                                || state.expanded_group != old_expanded
                            {
                                donut_dirty = true;
                            }
                        }
                    }

                    unsafe {
                        use windows::Win32::UI::WindowsAndMessaging::{LoadCursorW, SetCursor, IDC_ARROW, IDC_HAND, IDC_SIZEALL};
                        if popup_drag.is_some() {
                            if let Ok(cur) = LoadCursorW(None, IDC_SIZEALL) {
                                SetCursor(cur);
                                cursor_is_hand = false;
                            }
                        } else if is_hand {
                            if let Ok(cur) = LoadCursorW(None, IDC_HAND) {
                                SetCursor(cur);
                                cursor_is_hand = true;
                            }
                        } else if cursor_is_hand {
                            if let Ok(cur) = LoadCursorW(None, IDC_ARROW) {
                                SetCursor(cur);
                                cursor_is_hand = false;
                            }
                        }
                    }

                    g
                }
                HookEvent::MouseUp { btn, x, y, mods } => {
                    if popup_drag.is_some() {
                        popup_drag = None;
                    }
                    let g = detector.mouse_up(btn, x, y, mods);
                    if donut_state.is_none() && progress_tracker.is_none() && password_card_state.is_none() && reduce_size_card_state.is_none() && bulk_rename_card_state.is_none() && qr_card_state.is_none() && quick_qr_card_state.is_none() {
                        if let Some(sel) = fileuc_daemon::explorer::get_foreground_selected_files() {
                            order_tracker.update_selection(&sel);
                        }
                    }
                    g
                }
                HookEvent::KeyDown { mods } => detector.key_down(mods),
                HookEvent::KeyUp { mods } => detector.key_up(mods),
                HookEvent::KeyAction { vk, ch, mods } => {
                    if let Some(ref mut card) = quick_qr_card_state {
                        quick_qr_card_dirty = true;
                        if vk == 0x1B { // VK_ESCAPE
                            dismiss_quick_qr_card(&mut quick_qr_card_state, &mut hud);
                        } else if vk == 0x08 { // VK_BACK
                            card.pop_char();
                        } else if vk == 0x56 && mods.contains(Modifiers::CTRL) { // Ctrl + V
                            if let Some(clip_text) = get_clipboard_text() {
                                card.paste_text(&clip_text);
                            }
                        } else if (vk == 0x41 && mods.contains(Modifiers::CTRL)) || vk == 0x2E { // Ctrl + A or Delete
                            card.clear();
                        } else if vk == 0x0D { // VK_RETURN (Enter key saves to Desktop)
                            if let Some(ref matrix) = card.qr_matrix {
                                let prefix = match card.tab {
                                    QuickQrTab::Link => "Link",
                                    QuickQrTab::Text => "Text",
                                    QuickQrTab::Wifi => "WiFi",
                                };
                                if let Some(_path) = fileuc_daemon::save_qr_to_desktop(matrix, prefix) {
                                    card.saved_feedback_timer = Some(Instant::now());
                                    quick_qr_card_dirty = true;
                                }
                            }
                        } else if let Some(c) = ch {
                            if !mods.contains(Modifiers::CTRL) && !mods.contains(Modifiers::ALT) {
                                card.push_char(c);
                            }
                        }
                    } else if let Some(ref mut card) = password_card_state {
                        password_card_dirty = true;
                        if vk == 0x08 { // VK_BACK
                            card.pop_char();
                        } else if vk == 0x0D { // VK_RETURN
                            submit_password = true;
                        } else if vk == 0x1B { // VK_ESCAPE
                            dismiss_password_card(
                                &mut password_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if let Some(c) = ch {
                            card.push_char(c);
                        }
                    } else if let Some(ref mut card) = reduce_size_card_state {
                        reduce_size_card_dirty = true;
                        if vk == 0x08 { // VK_BACK
                            card.pop_char();
                        } else if vk == 0x0D { // VK_RETURN
                            submit_reduce_size = true;
                        } else if vk == 0x1B { // VK_ESCAPE
                            dismiss_reduce_size_card(
                                &mut reduce_size_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if let Some(c) = ch {
                            if c.is_ascii_digit() {
                                card.push_char(c);
                            }
                        }
                    } else if let Some(ref mut card) = bulk_rename_card_state {
                        bulk_rename_card_dirty = true;
                        if vk == 0x08 { // VK_BACK
                            card.pop_char();
                        } else if vk == 0x56 && mods.contains(Modifiers::CTRL) { // Ctrl + V
                            if let Some(clip_text) = get_clipboard_text() {
                                card.paste_text(&clip_text);
                            }
                        } else if (vk == 0x41 && mods.contains(Modifiers::CTRL)) || vk == 0x2E { // Ctrl + A or Delete
                            card.clear();
                        } else if vk == 0x0D { // VK_RETURN
                            submit_bulk_rename = true;
                        } else if vk == 0x1B { // VK_ESCAPE
                            dismiss_bulk_rename_card(
                                &mut bulk_rename_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        } else if let Some(c) = ch {
                            if !mods.contains(Modifiers::CTRL) && !mods.contains(Modifiers::ALT) {
                                card.push_char(c);
                            }
                        }
                    }
                    None
                }
                HookEvent::QuickQrTriggered => {
                    if cached_service_enabled {
                        if quick_qr_card_state.is_some() {
                            dismiss_quick_qr_card(&mut quick_qr_card_state, &mut hud);
                            continue;
                        }

                        if donut_state.is_some() {
                            donut_state = None;
                            hud.hide();
                            current_files.clear();
                            current_actions.clear();
                        }
                        if password_card_state.is_some() {
                            dismiss_password_card(
                                &mut password_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        }
                        if reduce_size_card_state.is_some() {
                            dismiss_reduce_size_card(
                                &mut reduce_size_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        }
                        if bulk_rename_card_state.is_some() {
                            dismiss_bulk_rename_card(
                                &mut bulk_rename_card_state,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        }
                        if qr_card_state.is_some() {
                            dismiss_qr_card(
                                &mut qr_card_state,
                                &mut share_server,
                                &mut pinggy_tunnel,
                                &mut cloudflare_tunnel,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        }

                        let mut pt = windows::Win32::Foundation::POINT::default();
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
                        }
                        hud_center = hud.position_at_center(pt.x, pt.y);
                        if hud.size() != pixmap.width() {
                            pixmap = tiny_skia::Pixmap::new(hud.size(), hud.size()).unwrap();
                        }

                        let mut card = QuickQrCardState::new(hud.dpi_scale());
                        let cached_ssid = detected_wifi_ssid.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        card.detected_ssid = cached_ssid;
                        if card.detected_ssid.is_none() {
                            let wifi_clone = detected_wifi_ssid.clone();
                            std::thread::spawn(move || {
                                if let Some(ssid) = fileuc_daemon::wifi::detect_wifi_ssid() {
                                    *wifi_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(ssid);
                                }
                            });
                        }
                        let (l, t, w, h) = card.card_bounds(hud_center.0 as f32, hud_center.1 as f32);
                        set_mouse_capture_rect(Some((l as i32, t as i32, (l + w) as i32, (t + h) as i32)));
                        set_key_capture(true);
                        render_quick_qr_card(&card, &cached_theme, &fonts, &mut pixmap);
                        hud.repaint(&pixmap);
                        hud.show();
                        quick_qr_card_state = Some(card);
                        quick_qr_card_dirty = false;
                    }
                    None
                }
            };

            if let Some(g) = gesture {
                match g {
                    GestureEvent::DragStarted { cursor_x, cursor_y } => {
                        if !service_enabled
                            || progress_tracker.is_some()
                            || password_card_state.is_some()
                            || qr_card_state.is_some()
                            || quick_qr_card_state.is_some()
                            || bulk_rename_card_state.is_some()
                            || reduce_size_card_state.is_some()
                            || popup_drag.is_some()
                        {
                            // Service paused, active conversion progress, or active modal dialog open; do not show HUD
                            continue;
                        }
                        pending_drag_ended = None;
                        donut_dirty = true;
                        hud_center = hud.position_at_center(cursor_x, cursor_y);
                        if hud.size() != pixmap.width() {
                            pixmap = tiny_skia::Pixmap::new(hud.size(), hud.size()).unwrap();
                        }

                        // Try to immediately detect selected files from Explorer
                        let detected = fileuc_daemon::explorer::get_selected_files();
                        if !detected.is_empty() {
                            order_tracker.update_selection(&detected);
                            current_files = order_tracker.order_files(&detected);
                            let has_ffmpeg = fileuc_convert::media::find_ffmpeg().is_some();
                            let actions = filter_actions(&current_files, has_ffmpeg);
                            current_actions = actions.clone();

                            let file_label = if current_files.len() == 1 {
                                current_files[0].extension().and_then(|s| s.to_str()).map(|ext| format!("1 {}", ext.to_uppercase()))
                            } else if current_files.len() > 1 {
                                Some(format!("{} FILES", current_files.len()))
                            } else {
                                None
                            };

                            let mut state = DonutState::new(
                                current_actions.clone(),
                                current_files.len(),
                                hud.dpi_scale(),
                            );
                            if let Some(lbl) = file_label {
                                state.set_file_label(lbl);
                            }
                            state.set_missing_plugins(get_missing_plugins());
                            donut_state = Some(state);
                        } else {
                            // If not yet detected via COM, show responsive donut ring with has_files = false
                            // so OLE DragEnter hit-tests immediately, but NO wrong format labels are drawn!
                            current_files.clear();
                            current_actions = vec![
                                ConversionAction::ToPng,
                                ConversionAction::ToJpg,
                                ConversionAction::ToPdf,
                                ConversionAction::CreateZip,
                            ];
                            let mut state = DonutState::new(
                                current_actions.clone(),
                                0,
                                hud.dpi_scale(),
                            );
                            state.has_files = false;
                            state.set_missing_plugins(get_missing_plugins());
                            donut_state = Some(state);
                        }
                        hud.show();
                    }
                    GestureEvent::DragEnded => {
                        if progress_tracker.is_none()
                            && qr_card_state.is_none()
                            && password_card_state.is_none()
                            && quick_qr_card_state.is_none()
                            && bulk_rename_card_state.is_none()
                            && reduce_size_card_state.is_none()
                        {
                            let is_hovering = donut_state
                                .as_ref()
                                .and_then(|s| s.hovered_slice)
                                .is_some();
                            if is_hovering {
                                // Mouse released while on a slice: give Windows OLE 250ms to deliver Dropped
                                pending_drag_ended = Some(Instant::now());
                            } else {
                                hud.hide();
                                donut_state = None;
                                current_files.clear();
                                current_actions.clear();
                                pending_drag_ended = None;
                            }
                        }
                    }
                }
            }
        }

        // 3. Process HUD Drop Events
        while let Ok(event) = drop_rx.try_recv() {
            match event {
                DropTargetEvent::FilesEntered(files) => {
                    if progress_tracker.is_some()
                        || password_card_state.is_some()
                        || qr_card_state.is_some()
                        || quick_qr_card_state.is_some()
                        || bulk_rename_card_state.is_some()
                        || reduce_size_card_state.is_some()
                    {
                        continue;
                    }
                    donut_dirty = true;
                    current_files = order_tracker.order_files(&files);
                    let has_ffmpeg = fileuc_convert::media::find_ffmpeg().is_some();
                    let actions = filter_actions(&current_files, has_ffmpeg);
                    current_actions = actions.clone();

                    let file_label = if files.len() == 1 {
                        files[0].extension().and_then(|s| s.to_str()).map(|ext| format!("1 {}", ext.to_uppercase()))
                    } else if files.len() > 1 {
                        Some(format!("{} FILES", files.len()))
                    } else {
                        None
                    };

                    if let Some(ref mut state) = donut_state {
                        state.update_content(actions, files.len(), file_label);
                        state.set_missing_plugins(get_missing_plugins());
                    } else {
                        let mut state = DonutState::new(actions, files.len(), hud.dpi_scale());
                        if let Some(lbl) = file_label {
                            state.set_file_label(lbl);
                        }
                        state.set_missing_plugins(get_missing_plugins());
                        donut_state = Some(state);
                    }
                }
                DropTargetEvent::DragOver { x, y } => {
                    if progress_tracker.is_some()
                        || password_card_state.is_some()
                        || qr_card_state.is_some()
                        || quick_qr_card_state.is_some()
                        || bulk_rename_card_state.is_some()
                        || reduce_size_card_state.is_some()
                    {
                        continue;
                    }
                    if let Some(ref mut state) = donut_state {
                        donut_dirty = true;
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let dpi_scale = hud.dpi_scale();
                        let pop_out = 7.0 * dpi_scale;
                        let _ = state.track_pointer(rel_x, rel_y, pop_out);
                    }
                }
                DropTargetEvent::DragLeft => {
                    if progress_tracker.is_some()
                        || password_card_state.is_some()
                        || qr_card_state.is_some()
                        || quick_qr_card_state.is_some()
                        || bulk_rename_card_state.is_some()
                        || reduce_size_card_state.is_some()
                    {
                        continue;
                    }
                    if let Some(ref mut state) = donut_state {
                        donut_dirty = true;
                        state.set_hovered(None);
                        state.set_center_hot(false);
                    }
                }
                DropTargetEvent::NoFiles => {
                    // Do not hide HUD while the user is actively dragging with Ctrl!
                }
                DropTargetEvent::Dropped { x, y } => {
                    if progress_tracker.is_some()
                        || password_card_state.is_some()
                        || qr_card_state.is_some()
                        || quick_qr_card_state.is_some()
                        || bulk_rename_card_state.is_some()
                        || reduce_size_card_state.is_some()
                    {
                        continue;
                    }
                    pending_drag_ended = None;
                    let rel_x = (x - hud_center.0) as f32;
                    let rel_y = (y - hud_center.1) as f32;
                    let dpi_scale = hud.dpi_scale();
                    let pop_out = 7.0 * dpi_scale;

                    let hit = if let Some(ref mut state) = donut_state {
                        state.track_pointer(rel_x, rel_y, pop_out)
                    } else {
                        HitResult::Outside
                    };

                    donut_state = None;

                    match hit {
                        HitResult::SubSlice { group_idx, sub_idx } => {
                            if group_idx < current_actions.len() {
                                let parent_action = current_actions[group_idx];
                                if let Some(children) = fileuc_hud::smart_filter::get_action_children(parent_action) {
                                    if sub_idx < children.len() {
                                        let action = children[sub_idx];
                                        if get_missing_plugins().contains(&action) {
                                            let st = current_settings.lock().unwrap().clone();
                                            settings_controller.show_tab(st, 1);
                                            hud.hide();
                                            current_files.clear();
                                            current_actions.clear();
                                        } else {
                                            let files = order_tracker.order_files(&current_files);
                                            let settings = Arc::new(current_settings.lock().unwrap().clone());
                                            execute_conversion_action(
                                                action,
                                                files,
                                                settings,
                                                hud.dpi_scale(),
                                                hud_center,
                                                &mut password_card_state,
                                                &mut password_card_dirty,
                                                &mut reduce_size_card_state,
                                                &mut reduce_size_card_dirty,
                                                &mut bulk_rename_card_state,
                                                &mut bulk_rename_card_dirty,
                                                &mut qr_card_state,
                                                &mut share_server,
                                                &mut progress_tracker,
                                                &mut conversion_in_flight,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        HitResult::Slice(idx) if idx < current_actions.len() => {
                            let action = current_actions[idx];
                            if get_missing_plugins().contains(&action) {
                                let st = current_settings.lock().unwrap().clone();
                                settings_controller.show_tab(st, 1);
                                hud.hide();
                                current_files.clear();
                                current_actions.clear();
                            } else {
                                let files = order_tracker.order_files(&current_files);
                                let settings = Arc::new(current_settings.lock().unwrap().clone());
                                execute_conversion_action(
                                    action,
                                    files,
                                    settings,
                                    hud.dpi_scale(),
                                    hud_center,
                                    &mut password_card_state,
                                    &mut password_card_dirty,
                                    &mut reduce_size_card_state,
                                    &mut reduce_size_card_dirty,
                                    &mut bulk_rename_card_state,
                                    &mut bulk_rename_card_dirty,
                                    &mut qr_card_state,
                                    &mut share_server,
                                    &mut progress_tracker,
                                    &mut conversion_in_flight,
                                );
                            }
                        }
                        _ => {
                            hud.hide();
                            current_files.clear();
                            current_actions.clear();
                        }
                    }
                }
            }
        }

        // 3.5. Timeout for pending drag ended
        if let Some(ended_at) = pending_drag_ended {
            if ended_at.elapsed() >= Duration::from_millis(250) {
                pending_drag_ended = None;
                if progress_tracker.is_none() && password_card_state.is_none() && reduce_size_card_state.is_none() && bulk_rename_card_state.is_none() {
                    let mut triggered = false;
                    if let Some(ref state) = donut_state {
                        let chosen_action = if let (Some(g), Some(sub)) = (state.expanded_group, state.sub_hover) {
                            if g < current_actions.len() {
                                if let Some(children) = fileuc_hud::smart_filter::get_action_children(current_actions[g]) {
                                    if sub < children.len() {
                                        Some(children[sub])
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else if let Some(idx) = state.hovered_slice {
                            if idx < current_actions.len() {
                                Some(current_actions[idx])
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some(action) = chosen_action {
                            if !current_files.is_empty() {
                                if get_missing_plugins().contains(&action) {
                                    let st = current_settings.lock().unwrap().clone();
                                    settings_controller.show_tab(st, 1);
                                    hud.hide();
                                    current_files.clear();
                                    current_actions.clear();
                                    triggered = true;
                                } else {
                                    let files = order_tracker.order_files(&current_files);
                                    let settings = Arc::new(current_settings.lock().unwrap().clone());
                                    execute_conversion_action(
                                        action,
                                        files,
                                        settings,
                                        hud.dpi_scale(),
                                        hud_center,
                                        &mut password_card_state,
                                        &mut password_card_dirty,
                                        &mut reduce_size_card_state,
                                        &mut reduce_size_card_dirty,
                                        &mut bulk_rename_card_state,
                                        &mut bulk_rename_card_dirty,
                                        &mut qr_card_state,
                                        &mut share_server,
                                        &mut progress_tracker,
                                        &mut conversion_in_flight,
                                    );
                                    triggered = true;
                                }
                            }
                        }
                    }
                    if !triggered {
                        hud.hide();
                        donut_state = None;
                        current_files.clear();
                        current_actions.clear();
                    } else {
                        donut_state = None;
                    }
                }
            }
        }

        // 3.8. Process Password Submission (Protect PDF / Unlock PDF)
        if submit_password {
            if let Some(ref mut card) = password_card_state {
                let pwd = card.password_input.clone();
                let src_path = card.file_path.clone();
                let settings = { current_settings.lock().unwrap().clone() };
                let mode = card.mode;

                if pwd.is_empty() {
                    card.set_error("Password cannot be empty");
                    password_card_dirty = true;
                } else {
                    match mode {
                        PasswordCardMode::Lock => {
                            let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(&src_path, "_locked", "pdf", None);
                            match fileuc_convert::pdf::lock_pdf(&src_path, &pwd, &out_pdf) {
                                Ok(()) => {
                                    let _final_out = fileuc_daemon::dispatcher::apply_output_mode(out_pdf, &settings.output_mode);
                                    set_key_capture(false);
                                    set_mouse_capture_rect(None);
                                    password_card_state = None;
                                    let mut tracker = ProgressTracker::new();
                                    tracker.mark_success();
                                    progress_tracker = Some(tracker);
                                }
                                Err(e) => {
                                    card.set_error(&e.to_string());
                                    password_card_dirty = true;
                                }
                            }
                        }
                        PasswordCardMode::Unlock => {
                            let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(&src_path, "_unlocked", "pdf", Some("_locked"));
                            match fileuc_convert::pdf::unlock_pdf(&src_path, Some(&pwd), &out_pdf) {
                                Ok(true) => {
                                    let _final_out = fileuc_daemon::dispatcher::apply_output_mode(out_pdf, &settings.output_mode);
                                    set_key_capture(false);
                                    set_mouse_capture_rect(None);
                                    password_card_state = None;
                                    let mut tracker = ProgressTracker::new();
                                    tracker.mark_success();
                                    progress_tracker = Some(tracker);
                                }
                                Ok(false) | Err(_) => {
                                    card.set_error("Incorrect password: document is highly secured.");
                                    password_card_dirty = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3.9. Process Reduce Size Submission
        if submit_reduce_size {
            if let Some(ref mut card) = reduce_size_card_state {
                if let Some(target) = card.target_bytes() {
                    let files = current_files.clone();
                    let settings = Arc::new(current_settings.lock().unwrap_or_else(|e| e.into_inner()).clone());
                    set_key_capture(false);
                    set_mouse_capture_rect(None);
                    reduce_size_card_state = None;

                    let tracker = ProgressTracker::new();
                    progress_tracker = Some(tracker);

                    let progress_val = Arc::new(AtomicU32::new(0));
                    let handle = fileuc_daemon::dispatcher::dispatch_reduce_size(
                        files,
                        settings,
                        target,
                        progress_val.clone(),
                    );

                    conversion_in_flight = Some((handle, progress_val));
                } else {
                    reduce_size_card_dirty = true;
                }
            }
        }

        // 3.10. Process Bulk Rename Submission
        if submit_bulk_rename {
            if let Some(ref mut card) = bulk_rename_card_state {
                let base_name = card.input_text.trim().to_string();
                if !base_name.is_empty() {
                    let files = current_files.clone();
                    set_key_capture(false);
                    set_mouse_capture_rect(None);
                    bulk_rename_card_state = None;

                    match fileuc_convert::renamer::bulk_rename(&files, &base_name) {
                        Ok(_) => {
                            let mut tracker = ProgressTracker::new();
                            tracker.mark_success();
                            progress_tracker = Some(tracker);
                        }
                        Err(e) => {
                            let mut tracker = ProgressTracker::new();
                            tracker.mark_failure(e.to_string());
                            progress_tracker = Some(tracker);
                        }
                    }
                } else {
                    bulk_rename_card_dirty = true;
                }
            }
        }

        // 4. Update in-flight conversion progress
        if let Some((ref handle, ref p_val)) = conversion_in_flight {
            let frac = p_val.load(Ordering::Relaxed) as f32 / 1000.0;
            if let Some(ref mut tracker) = progress_tracker {
                tracker.update(frac);
            }
            if handle.is_finished() {
                let (h, _) = conversion_in_flight.take().unwrap();
                match h.join() {
                    Ok(res) => {
                        if let Some(ref mut tracker) = progress_tracker {
                            if res.errors.is_empty() {
                                tracker.mark_success();
                            } else {
                                let (is_engine_missing, err_msg) = if let Some(e) = res.errors.first() {
                                    (matches!(e.1, fileuc_types::ConvertError::EngineMissing(_)), e.1.to_string())
                                } else {
                                    (false, "Conversion failed".into())
                                };
                                if is_engine_missing {
                                    let st = current_settings.lock().unwrap().clone();
                                    settings_controller.show_tab(st, 1);
                                    progress_tracker = None;
                                    hud.hide();
                                    current_files.clear();
                                    current_actions.clear();
                                } else {
                                    tracker.mark_failure(err_msg);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        if let Some(ref mut tracker) = progress_tracker {
                            tracker.mark_failure("Conversion task encountered an unexpected error".into());
                        }
                    }
                }
            }
        }

        // 4.3. Process Cloudflared Download Events
        if let Some(ref rx) = cloudflared_dl_rx {
            while let Ok(progress) = rx.try_recv() {
                if let Some(ref mut card) = qr_card_state {
                    match progress {
                        fileuc_daemon::cloudflared::DownloadProgress::Starting => {
                            card.download_pct = Some(0);
                            qr_card_dirty = true;
                        }
                        fileuc_daemon::cloudflared::DownloadProgress::Progress(pct) => {
                            card.download_pct = Some(pct);
                            qr_card_dirty = true;
                        }
                        fileuc_daemon::cloudflared::DownloadProgress::Completed => {
                            card.is_downloading = false;
                            card.is_cloudflared_installed = true;
                            card.download_pct = None;
                            qr_card_dirty = true;
                            if card.tab == QrShareTab::Internet && card.internet_url.is_none() {
                                if let Some(ref s) = share_server {
                                    let port = s.port;
                                    let path = s.url.split('/').skip(3).collect::<Vec<&str>>().join("/");
                                    match fileuc_daemon::cloudflared::CloudflareTunnel::start(port, &format!("/{}", path)) {
                                        Ok(tunnel) => {
                                            cloudflare_tunnel = Some(tunnel);
                                        }
                                        Err(e) => {
                                            card.set_tunnel_error(&e);
                                        }
                                    }
                                }
                            }
                        }
                        fileuc_daemon::cloudflared::DownloadProgress::Error(err) => {
                            card.is_downloading = false;
                            card.download_pct = None;
                            card.set_tunnel_error(&err);
                            qr_card_dirty = true;
                        }
                    }
                }
            }
        }

        // 4.4. Process Cloudflare Tunnel URL
        if let Some(ref tunnel) = cloudflare_tunnel {
            while let Ok(res) = tunnel.rx_url.try_recv() {
                if let Some(ref mut card) = qr_card_state {
                    match res {
                        Ok(url) => {
                            card.set_internet_url(&url);
                        }
                        Err(err) => {
                            card.set_tunnel_error(&err);
                        }
                    }
                    qr_card_dirty = true;
                }
            }
        }

        // 4.4.1. Process Pinggy Tunnel URL
        if let Some(ref tunnel) = pinggy_tunnel {
            while let Ok(res) = tunnel.rx_url.try_recv() {
                if let Some(ref mut card) = qr_card_state {
                    match res {
                        Ok(url) => {
                            card.set_internet_url(&url);
                        }
                        Err(err) => {
                            card.set_tunnel_error(&err);
                        }
                    }
                    qr_card_dirty = true;
                }
            }
        }

        // 4.5. Process Share Server Events
        if let Some(ref server) = share_server {
            while let Ok(event) = server.rx.try_recv() {
                if let Some(ref mut card) = qr_card_state {
                    match event {
                        ShareServerEvent::ClientConnected => {
                            card.set_scanned();
                            qr_card_dirty = true;
                        }
                        ShareServerEvent::DownloadStarted => {
                            card.set_sending();
                            qr_card_dirty = true;
                        }
                        ShareServerEvent::DownloadFinished => {
                            card.set_success();
                            qr_card_dirty = true;
                        }
                        ShareServerEvent::Error(err) => {
                            card.set_failed(&err);
                            qr_card_dirty = true;
                        }
                    }
                }
            }
        }

        // 4.6. Enforce 5-minute timeout when QR code is not scanned, or scanned but no download taking place
        if let Some(ref card) = qr_card_state {
            use fileuc_hud::qr_card::QrShareStatus;
            let timed_out = match card.status {
                QrShareStatus::WaitingForScan => {
                    card.session_start.elapsed() >= Duration::from_secs(300)
                }
                QrShareStatus::Scanned => {
                    let start = card.scanned_time.unwrap_or(card.session_start);
                    start.elapsed() >= Duration::from_secs(300)
                }
                QrShareStatus::Sending => false, // Active transfer in progress: keep alive!
                QrShareStatus::Success => false, // Handled by 1.5s auto-dismiss
                QrShareStatus::Failed(_) => card.session_start.elapsed() >= Duration::from_secs(300),
            };

            if timed_out {
                dismiss_qr_card(
                    &mut qr_card_state,
                    &mut share_server,
                    &mut pinggy_tunnel,
                    &mut cloudflare_tunnel,
                    &mut hud,
                    &mut current_files,
                    &mut current_actions,
                );
            }
        }

        // 5. Render HUD Frame
        let theme = &cached_theme;

        if let Some(ref mut card) = quick_qr_card_state {
            if card.detected_ssid.is_none() {
                if let Some(ref ssid) = *detected_wifi_ssid.lock().unwrap_or_else(|e| e.into_inner()) {
                    card.detected_ssid = Some(ssid.clone());
                    card.update_qr();
                    quick_qr_card_dirty = true;
                }
            }
            let animating = card.tick(1.0 / 60.0);
            if animating || quick_qr_card_dirty {
                render_quick_qr_card(card, theme, &fonts, &mut pixmap);
                hud.repaint(&pixmap);
            }
        } else if let Some(ref mut card) = password_card_state {
            let animating = card.tick(1.0 / 60.0);
            if animating || password_card_dirty {
                render_password_card(card, theme, &fonts, &mut pixmap);
                hud.repaint(&pixmap);
            }
        } else if let Some(ref mut card) = reduce_size_card_state {
            let animating = card.tick(1.0 / 60.0);
            if animating || reduce_size_card_dirty {
                render_reduce_size_card(card, theme, &fonts, &mut pixmap);
                hud.repaint(&pixmap);
            }
        } else if let Some(ref mut card) = bulk_rename_card_state {
            let animating = card.tick(1.0 / 60.0);
            if animating || bulk_rename_card_dirty {
                render_bulk_rename_card(card, theme, &fonts, &mut pixmap);
                hud.repaint(&pixmap);
            }
        } else if let Some(ref mut card) = qr_card_state {
            let animating = card.tick(1.0 / 60.0);
            if animating || qr_card_dirty {
                render_qr_card(card, theme, &fonts, &mut pixmap);
                hud.repaint(&pixmap);
            }
            if card.is_finished() {
                dismiss_qr_card(
                    &mut qr_card_state,
                    &mut share_server,
                    &mut pinggy_tunnel,
                    &mut cloudflare_tunnel,
                    &mut hud,
                    &mut current_files,
                    &mut current_actions,
                );
            }
        } else if let Some(ref mut tracker) = progress_tracker {
            let animating = tracker.tick(1.0 / 60.0);
            if animating || progress_dirty {
                render_progress(tracker, theme, &fonts, hud.dpi_scale(), &mut pixmap);
                hud.repaint(&pixmap);
            }
            if tracker.fade_elapsed() {
                progress_tracker = None;
                set_mouse_capture_rect(None);
                hud.hide();
                current_files.clear();
                current_actions.clear();
            }
        } else if let Some(ref mut state) = donut_state {
            let animating = state.tick(1.0 / 60.0);
            if animating || donut_dirty {
                render_donut(state, theme, &fonts, &mut pixmap);
                hud.repaint(&pixmap);
            }
        }

        let is_active = donut_state.is_some()
            || progress_tracker.is_some()
            || password_card_state.is_some()
            || reduce_size_card_state.is_some()
            || bulk_rename_card_state.is_some()
            || qr_card_state.is_some()
            || quick_qr_card_state.is_some()
            || pending_drag_ended.is_some()
            || conversion_in_flight.is_some()
            || cloudflared_dl_rx.as_ref().map_or(false, |_| qr_card_state.as_ref().map_or(false, |c| c.is_downloading));

        if was_active && !is_active {
            fileuc_daemon::trim_working_set();
        }
        was_active = is_active;

        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                MsgWaitForMultipleObjectsEx, MWMO_INPUTAVAILABLE, QS_ALLINPUT,
            };
            let timeout = if is_active { 16 } else { 250 };
            let _ = MsgWaitForMultipleObjectsEx(None, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }

    settings_controller.quit();
    drop(hud);

    unsafe {
        windows::Win32::System::Ole::OleUninitialize();
    }
}

fn execute_conversion_action(
    action: ConversionAction,
    files: Vec<PathBuf>,
    settings: Arc<fileuc_settings::store::AppSettings>,
    dpi_scale: f32,
    hud_center: (i32, i32),
    password_card_state: &mut Option<PasswordCardState>,
    password_card_dirty: &mut bool,
    reduce_size_card_state: &mut Option<ReduceSizeCardState>,
    reduce_size_card_dirty: &mut bool,
    bulk_rename_card_state: &mut Option<BulkRenameCardState>,
    bulk_rename_card_dirty: &mut bool,
    qr_card_state: &mut Option<QrCardState>,
    share_server: &mut Option<ShareServerHandle>,
    progress_tracker: &mut Option<ProgressTracker>,
    conversion_in_flight: &mut Option<(JoinHandle<ConversionResult>, Arc<AtomicU32>)>,
) {
    if action == ConversionAction::ReduceSize {
        let total_size_bytes: u64 = files.iter()
            .filter_map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
            .sum();
        let card = ReduceSizeCardState::new(
            files.len(),
            total_size_bytes,
            dpi_scale,
        );
        let (l, t, w, h) = card.card_bounds(hud_center.0 as f32, hud_center.1 as f32);
        set_mouse_capture_rect(Some((l as i32, t as i32, (l + w) as i32, (t + h) as i32)));
        set_key_capture(true);
        *reduce_size_card_state = Some(card);
        *reduce_size_card_dirty = true;
    } else if action == ConversionAction::BulkRename {
        let sample_ext = files.first().and_then(|f| f.extension()).and_then(|e| e.to_str()).unwrap_or("");
        let card = BulkRenameCardState::new(
            files.len(),
            sample_ext,
            dpi_scale,
        );
        let (l, t, w, h) = card.card_bounds(hud_center.0 as f32, hud_center.1 as f32);
        set_mouse_capture_rect(Some((l as i32, t as i32, (l + w) as i32, (t + h) as i32)));
        set_key_capture(true);
        *bulk_rename_card_state = Some(card);
        *bulk_rename_card_dirty = true;
    } else if action == ConversionAction::OfficeGroup {
        return execute_conversion_action(
            ConversionAction::PdfToDocx,
            files,
            settings,
            dpi_scale,
            hud_center,
            password_card_state,
            password_card_dirty,
            reduce_size_card_state,
            reduce_size_card_dirty,
            bulk_rename_card_state,
            bulk_rename_card_dirty,
            qr_card_state,
            share_server,
            progress_tracker,
            conversion_in_flight,
        );
    } else if action == ConversionAction::LockPdf {
        if let Some(first) = files.first() {
            let card = PasswordCardState::new(
                first.clone(),
                dpi_scale,
            );
            let (l, t, w, h) = card.card_bounds(hud_center.0 as f32, hud_center.1 as f32);
            set_mouse_capture_rect(Some((l as i32, t as i32, (l + w) as i32, (t + h) as i32)));
            set_key_capture(true);
            *password_card_state = Some(card);
            *password_card_dirty = true;
        }
    } else if action == ConversionAction::UnlockPdf {
        if let Some(first) = files.first() {
            let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(first, "_unlocked", "pdf", Some("_locked"));
            // Step 1: Silent unlock attempt (handles permission/owner locks with empty password instantly!)
            match fileuc_convert::pdf::unlock_pdf(first, None, &out_pdf) {
                Ok(true) => {
                    // Instantly unlocked without needing password prompt!
                    let _final_out = fileuc_daemon::dispatcher::apply_output_mode(out_pdf, &settings.output_mode);
                    let mut tracker = ProgressTracker::new();
                    tracker.mark_success();
                    *progress_tracker = Some(tracker);
                    let half = (44.0 * dpi_scale) as i32;
                    set_mouse_capture_rect(Some((hud_center.0 - half, hud_center.1 - half, hud_center.0 + half, hud_center.1 + half)));
                    set_key_capture(false);
                }
                Ok(false) => {
                    // Document requires open password: open PasswordCard in Unlock mode!
                    let card = PasswordCardState::new_with_mode(
                        first.clone(),
                        dpi_scale,
                        PasswordCardMode::Unlock,
                    );
                    let (l, t, w, h) = card.card_bounds(hud_center.0 as f32, hud_center.1 as f32);
                    set_mouse_capture_rect(Some((l as i32, t as i32, (l + w) as i32, (t + h) as i32)));
                    set_key_capture(true);
                    *password_card_state = Some(card);
                    *password_card_dirty = true;
                }
                Err(e) => {
                    let mut tracker = ProgressTracker::new();
                    tracker.mark_failure(e.to_string());
                    *progress_tracker = Some(tracker);
                    let half = (44.0 * dpi_scale) as i32;
                    set_mouse_capture_rect(Some((hud_center.0 - half, hud_center.1 - half, hud_center.0 + half, hud_center.1 + half)));
                    set_key_capture(false);
                }
            }
        }
    } else if action == ConversionAction::Share {
        let share_init = start_share_server(&files).and_then(|handle| {
            QrCardState::new(
                &handle.url,
                &handle.file_name,
                handle.file_size,
                dpi_scale,
            )
            .map(|mut state| {
                state.is_cloudflared_installed = fileuc_daemon::cloudflared::is_cloudflared_installed();
                (handle, state)
            })
        });

        match share_init {
            Ok((handle, state)) => {
                let (l, t, w, h) = state.card_bounds(hud_center.0 as f32, hud_center.1 as f32);
                set_mouse_capture_rect(Some((l as i32, t as i32, (l + w) as i32, (t + h) as i32)));
                set_key_capture(false);
                *qr_card_state = Some(state);
                *share_server = Some(handle);
            }
            Err(err) => {
                let mut tracker = ProgressTracker::new();
                tracker.mark_failure(err);
                *progress_tracker = Some(tracker);
                let half = (44.0 * dpi_scale) as i32;
                set_mouse_capture_rect(Some((hud_center.0 - half, hud_center.1 - half, hud_center.0 + half, hud_center.1 + half)));
                set_key_capture(false);
            }
        }
    } else {
        let tracker = ProgressTracker::new();
        *progress_tracker = Some(tracker);
        let half = (44.0 * dpi_scale) as i32;
        set_mouse_capture_rect(Some((hud_center.0 - half, hud_center.1 - half, hud_center.0 + half, hud_center.1 + half)));
        set_key_capture(false);

        let progress_val = Arc::new(AtomicU32::new(0));
        let p_clone = progress_val.clone();

        let handle = dispatch(
            ConversionRequest {
                files,
                action,
                settings,
            },
            move |p| {
                let int_val = (p * 1000.0) as u32;
                p_clone.store(int_val, Ordering::Relaxed);
            },
        );

        *conversion_in_flight = Some((handle, progress_val));
    }
}

fn dismiss_password_card(
    password_card_state: &mut Option<PasswordCardState>,
    hud: &mut HudWindow,
    files: &mut Vec<PathBuf>,
    actions: &mut Vec<ConversionAction>,
) {
    set_key_capture(false);
    set_mouse_capture_rect(None);
    *password_card_state = None;
    hud.hide();
    files.clear();
    actions.clear();
}

fn dismiss_reduce_size_card(
    reduce_size_card_state: &mut Option<ReduceSizeCardState>,
    hud: &mut HudWindow,
    files: &mut Vec<PathBuf>,
    actions: &mut Vec<ConversionAction>,
) {
    set_key_capture(false);
    set_mouse_capture_rect(None);
    *reduce_size_card_state = None;
    hud.hide();
    files.clear();
    actions.clear();
}

fn dismiss_bulk_rename_card(
    bulk_rename_card_state: &mut Option<BulkRenameCardState>,
    hud: &mut HudWindow,
    files: &mut Vec<PathBuf>,
    actions: &mut Vec<ConversionAction>,
) {
    set_key_capture(false);
    set_mouse_capture_rect(None);
    *bulk_rename_card_state = None;
    hud.hide();
    files.clear();
    actions.clear();
}

fn dismiss_qr_card(
    qr_card_state: &mut Option<QrCardState>,
    share_server: &mut Option<ShareServerHandle>,
    pinggy_tunnel: &mut Option<fileuc_daemon::pinggy::PinggyTunnel>,
    cloudflare_tunnel: &mut Option<fileuc_daemon::cloudflared::CloudflareTunnel>,
    hud: &mut HudWindow,
    files: &mut Vec<PathBuf>,
    actions: &mut Vec<ConversionAction>,
) {
    set_mouse_capture_rect(None);
    if let Some(mut c) = cloudflare_tunnel.take() {
        c.stop();
    }
    if let Some(mut p) = pinggy_tunnel.take() {
        p.stop();
    }
    if let Some(mut s) = share_server.take() {
        s.stop();
    }
    *qr_card_state = None;
    hud.hide();
    files.clear();
    actions.clear();
}

fn dismiss_quick_qr_card(
    quick_qr_card_state: &mut Option<QuickQrCardState>,
    hud: &mut HudWindow,
) {
    set_key_capture(false);
    set_mouse_capture_rect(None);
    *quick_qr_card_state = None;
    hud.hide();
}
