#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fileuc_daemon::autostart::set_auto_start;
use fileuc_daemon::dispatcher::{dispatch, ConversionRequest, ConversionResult};
use fileuc_daemon::ipc::{send_message, start_server, IpcMessage};
use fileuc_daemon::single_instance::acquire;
use fileuc_hook::gesture::{GestureDetector, GestureEvent, Modifiers};
use fileuc_hook::mouse_hook::{install, set_key_capture, set_mouse_capture_rect, HookEvent};
use fileuc_daemon::share_server::{start_share_server, ShareServerEvent, ShareServerHandle};
use fileuc_hud::donut_geometry::{DonutLayout, HitResult};
use fileuc_hud::drop_target::DropTargetEvent;
use fileuc_hud::font::FontContext;
use fileuc_hud::hud_window::HudWindow;
use fileuc_hud::progress_ring::ProgressTracker;
use fileuc_hud::password_card::{render_password_card, PasswordCardState};
use fileuc_hud::qr_card::{render_qr_card, QrCardState};
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

fn main() {
    if std::env::var("SLINT_BACKEND").is_err() {
        std::env::set_var("SLINT_BACKEND", "winit-software");
    }
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = OleInitialize(None);
    }

    let mutex_name = "FileUC_SingleInstance_Mutex";
    let pipe_name = "FileUC_Ipc_Pipe";

    let args: Vec<String> = std::env::args().collect();
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
            if send_message(pipe_name, IpcMessage::OpenSettings).is_ok() {
                return;
            }
            // If IPC pipe is not reachable, no instance is actively running; continue startup
            None
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

    // Apply auto-start state
    {
        let auto_start = current_settings.lock().unwrap().auto_start;
        let _ = set_auto_start(auto_start);
    }

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
    let quit_flag = should_quit.clone();
    let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    let _server = start_server(pipe_name, move |msg| match msg {
        IpcMessage::OpenSettings => {
            let st = settings_for_ipc.lock().unwrap().clone();
            ctrl_for_ipc.show(st);
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

    // If launched directly by user (not silent/autostart), open the settings window immediately!
    if !is_silent {
        let st = current_settings.lock().unwrap().clone();
        settings_controller.show(st);
    }

    // Create Mouse Hook
    let (hook_tx, hook_rx) = mpsc::channel();
    let _hook = install(hook_tx).expect("Failed to install mouse hook");

    // Create Unified HUD Window & Drop Target
    let (drop_tx, drop_rx) = mpsc::channel();
    let mut hud = HudWindow::new(drop_tx).expect("Failed to create HudWindow");

    // Create System Tray Event Channel & Icon (if enabled in settings)
    let (tray_tx, tray_rx) = mpsc::channel();
    let initial_service_enabled = current_settings.lock().unwrap().service_enabled;
    let mut tray: Option<fileuc_daemon::tray::TrayIcon> = if current_settings.lock().unwrap().show_tray_icon {
        Some(fileuc_daemon::tray::TrayIcon::new(tray_tx.clone(), initial_service_enabled))
    } else {
        None
    };

    // State machine
    let initial_mod = {
        let st = current_settings.lock().unwrap();
        match st.trigger_modifier.as_str() {
            "Alt" => Modifiers::ALT,
            "Shift" => Modifiers::SHIFT,
            _ => Modifiers::CTRL,
        }
    };
    let mut cached_service_enabled = initial_service_enabled;
    let mut cached_theme_choice = current_settings.lock().unwrap().theme;
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
    let mut password_card_state: Option<PasswordCardState> = None;
    let mut share_server: Option<ShareServerHandle> = None;
    let mut conversion_in_flight: Option<(JoinHandle<ConversionResult>, Arc<AtomicU32>)> = None;
    let mut hud_center = hud.hud_center();
    let mut pending_drag_ended: Option<Instant> = None;

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
        let mut password_card_dirty = false;
        let mut qr_card_dirty = false;
        let mut submit_password = false;

        // Sync trigger modifier, theme, tray and service state dynamically only when changed
        if settings_changed.swap(false, Ordering::Acquire) {
            let st = current_settings.lock().unwrap().clone();
            let new_mod = match st.trigger_modifier.as_str() {
                "Alt" => Modifiers::ALT,
                "Shift" => Modifiers::SHIFT,
                _ => Modifiers::CTRL,
            };
            detector.set_required_modifier(new_mod);
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
        if !service_enabled && donut_state.is_some() {
            donut_state = None;
            hud.hide();
            current_files.clear();
            current_actions.clear();
        }

        // Process Tray Events from dedicated Tray Window
        while let Ok(tray_event) = tray_rx.try_recv() {
            match tray_event {
                fileuc_daemon::tray::TrayEvent::OpenSettings => {
                    let st = current_settings.lock().unwrap().clone();
                    settings_controller.show(st);
                }
                fileuc_daemon::tray::TrayEvent::ToggleService => {
                    let new_val = {
                        let mut st = current_settings.lock().unwrap();
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

        // 2. Process Hook Events
        while let Ok(event) = hook_rx.try_recv() {
            let gesture = match event {
                HookEvent::MouseDown { btn, x, y, mods } => {
                    if let Some(ref mut card) = password_card_state {
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
                        }
                        // Clicking outside does NOT close anything!
                    } else if let Some(ref card) = qr_card_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let cx = hud.size() as f32 / 2.0;
                        let cy = hud.size() as f32 / 2.0;
                        if card.hit_test_close(rel_x, rel_y, cx, cy) {
                            dismiss_qr_card(
                                &mut qr_card_state,
                                &mut share_server,
                                &mut hud,
                                &mut current_files,
                                &mut current_actions,
                            );
                        }
                        // Clicking outside does NOT close anything!
                    }
                    detector.mouse_down(btn, x, y, mods)
                }
                HookEvent::MouseMove { x, y, mods } => {
                    let g = detector.mouse_move(x, y, mods);
                    let rel_x = (x - hud_center.0) as f32;
                    let rel_y = (y - hud_center.1) as f32;
                    let cx = hud.size() as f32 / 2.0;
                    let cy = hud.size() as f32 / 2.0;
                    let mut is_hand = false;

                    if let Some(ref mut card) = password_card_state {
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
                    } else if let Some(ref mut card) = qr_card_state {
                        let close_hov = card.hit_test_close(rel_x, rel_y, cx, cy);
                        if card.is_close_hovered != close_hov {
                            card.is_close_hovered = close_hov;
                            qr_card_dirty = true;
                        }
                        if close_hov {
                            is_hand = true;
                        }
                    } else if donut_state.is_some() && progress_tracker.is_none() {
                        let dpi_scale = hud.dpi_scale();
                        let base_inner_r = 63.0 * dpi_scale;
                        let base_outer_r = 150.0 * dpi_scale;
                        let pop_out = 7.0 * dpi_scale;
                        let layout = DonutLayout::new(base_inner_r, base_outer_r, current_actions.len());
                        let hit = layout.hit_test_with_slack(rel_x, rel_y, pop_out);
                        if let Some(ref mut state) = donut_state {
                            match hit {
                                HitResult::Slice(i) => state.set_hovered(Some(i)),
                                HitResult::CenterHole => state.set_center_hot(true),
                                HitResult::Outside => {
                                    state.set_hovered(None);
                                    state.set_center_hot(false);
                                }
                            }
                            donut_dirty = true;
                        }
                    }

                    unsafe {
                        use windows::Win32::UI::WindowsAndMessaging::{LoadCursorW, SetCursor, IDC_ARROW, IDC_HAND};
                        if is_hand {
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
                HookEvent::MouseUp { btn, x, y, mods } => detector.mouse_up(btn, x, y, mods),
                HookEvent::KeyDown { mods } => detector.key_down(mods),
                HookEvent::KeyUp { mods } => detector.key_up(mods),
                HookEvent::KeyAction { vk, ch } => {
                    if let Some(ref mut card) = password_card_state {
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
                    }
                    None
                }
            };

            if let Some(g) = gesture {
                match g {
                    GestureEvent::DragStarted { cursor_x, cursor_y } => {
                        if !service_enabled || password_card_state.is_some() || qr_card_state.is_some() {
                            // Service paused or active modal dialog open; do not show HUD
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
                            current_files = detected.clone();
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
                            donut_state = Some(state);
                        }
                        hud.show();
                    }
                    GestureEvent::DragEnded => {
                        if progress_tracker.is_none() && qr_card_state.is_none() && password_card_state.is_none() {
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
                    donut_dirty = true;
                    current_files = files.clone();
                    let has_ffmpeg = fileuc_convert::media::find_ffmpeg().is_some();
                    let actions = filter_actions(&files, has_ffmpeg);
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
                    } else {
                        let mut state = DonutState::new(actions, files.len(), hud.dpi_scale());
                        if let Some(lbl) = file_label {
                            state.set_file_label(lbl);
                        }
                        donut_state = Some(state);
                    }
                }
                DropTargetEvent::DragOver { x, y } => {
                    if let Some(ref mut state) = donut_state {
                        donut_dirty = true;
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let dpi_scale = hud.dpi_scale();
                        let base_inner_r = 63.0 * dpi_scale;
                        let base_outer_r = 150.0 * dpi_scale;
                        let pop_out = 7.0 * dpi_scale;
                        let layout = DonutLayout::new(base_inner_r, base_outer_r, current_actions.len());
                        let hit = layout.hit_test_with_slack(rel_x, rel_y, pop_out);
                        match hit {
                            HitResult::Slice(i) => state.set_hovered(Some(i)),
                            HitResult::CenterHole => state.set_center_hot(true),
                            HitResult::Outside => {
                                state.set_hovered(None);
                                state.set_center_hot(false);
                            }
                        }
                    }
                }
                DropTargetEvent::DragLeft => {
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
                    pending_drag_ended = None;
                    let rel_x = (x - hud_center.0) as f32;
                    let rel_y = (y - hud_center.1) as f32;
                    let dpi_scale = hud.dpi_scale();
                    let base_inner_r = 63.0 * dpi_scale;
                    let base_outer_r = 150.0 * dpi_scale;
                    let pop_out = 7.0 * dpi_scale;
                    let layout = DonutLayout::new(base_inner_r, base_outer_r, current_actions.len());
                    let hit = layout.hit_test_with_slack(rel_x, rel_y, pop_out);

                    donut_state = None;

                    match hit {
                        HitResult::Slice(idx) if idx < current_actions.len() => {
                            let action = current_actions[idx];
                            let files = current_files.clone();
                            let settings = Arc::new(current_settings.lock().unwrap().clone());
                            execute_conversion_action(
                                action,
                                files,
                                settings,
                                hud.dpi_scale(),
                                hud_center,
                                &mut password_card_state,
                                &mut password_card_dirty,
                                &mut qr_card_state,
                                &mut share_server,
                                &mut progress_tracker,
                                &mut conversion_in_flight,
                            );
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
                if progress_tracker.is_none() && password_card_state.is_none() {
                    let mut triggered = false;
                    if let Some(ref state) = donut_state {
                        if let Some(idx) = state.hovered_slice {
                            if idx < current_actions.len() && !current_files.is_empty() {
                                let action = current_actions[idx];
                                let files = current_files.clone();
                                let settings = Arc::new(current_settings.lock().unwrap().clone());
                                execute_conversion_action(
                                    action,
                                    files,
                                    settings,
                                    hud.dpi_scale(),
                                    hud_center,
                                    &mut password_card_state,
                                    &mut password_card_dirty,
                                    &mut qr_card_state,
                                    &mut share_server,
                                    &mut progress_tracker,
                                    &mut conversion_in_flight,
                                );
                                triggered = true;
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

        // 3.8. Process Password Submission (Protect PDF)
        if submit_password {
            if let Some(ref mut card) = password_card_state {
                let pwd = card.password_input.clone();
                let src_path = card.file_path.clone();
                let settings = { current_settings.lock().unwrap().clone() };

                if pwd.is_empty() {
                    card.set_error("Password cannot be empty");
                    password_card_dirty = true;
                } else {
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
                                let err_msg = res
                                    .errors
                                    .first()
                                    .map(|e| e.1.to_string())
                                    .unwrap_or_else(|| "Conversion failed".into());
                                tracker.mark_failure(err_msg);
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
                    &mut hud,
                    &mut current_files,
                    &mut current_actions,
                );
            }
        }

        // 5. Render HUD Frame
        let theme = &cached_theme;

        if let Some(ref mut card) = password_card_state {
            let animating = card.tick(1.0 / 60.0);
            if animating || password_card_dirty {
                render_password_card(card, theme, &fonts, &mut pixmap);
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
                    &mut hud,
                    &mut current_files,
                    &mut current_actions,
                );
            }
        } else if let Some(ref mut tracker) = progress_tracker {
            tracker.tick(1.0 / 60.0);
            render_progress(tracker, theme, &fonts, hud.dpi_scale(), &mut pixmap);
            hud.repaint(&pixmap);
            if tracker.fade_elapsed() {
                progress_tracker = None;
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
            || qr_card_state.is_some()
            || pending_drag_ended.is_some()
            || conversion_in_flight.is_some();

        if was_active && !is_active {
            fileuc_daemon::trim_working_set();
        }
        was_active = is_active;

        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                MsgWaitForMultipleObjectsEx, MWMO_INPUTAVAILABLE, QS_ALLINPUT,
            };
            let timeout = if is_active { 16 } else { 100 };
            let _ = MsgWaitForMultipleObjectsEx(None, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }

    settings_controller.quit();

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
    qr_card_state: &mut Option<QrCardState>,
    share_server: &mut Option<ShareServerHandle>,
    progress_tracker: &mut Option<ProgressTracker>,
    conversion_in_flight: &mut Option<(JoinHandle<ConversionResult>, Arc<AtomicU32>)>,
) {
    if action == ConversionAction::LockPdf {
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
    } else if action == ConversionAction::Share {
        let share_init = start_share_server(&files).and_then(|handle| {
            QrCardState::new(
                &handle.url,
                &handle.file_name,
                handle.file_size,
                dpi_scale,
            )
            .map(|state| (handle, state))
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
            }
        }
    } else {
        let tracker = ProgressTracker::new();
        *progress_tracker = Some(tracker);

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

fn dismiss_qr_card(
    qr_card_state: &mut Option<QrCardState>,
    share_server: &mut Option<ShareServerHandle>,
    hud: &mut HudWindow,
    files: &mut Vec<PathBuf>,
    actions: &mut Vec<ConversionAction>,
) {
    set_mouse_capture_rect(None);
    if let Some(mut s) = share_server.take() {
        s.stop();
    }
    *qr_card_state = None;
    hud.hide();
    files.clear();
    actions.clear();
}
