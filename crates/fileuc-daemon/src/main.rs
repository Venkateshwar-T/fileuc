use fileuc_daemon::autostart::set_auto_start;
use fileuc_daemon::dispatcher::{dispatch, ConversionRequest, ConversionResult};
use fileuc_daemon::ipc::{send_message, start_server, IpcMessage};
use fileuc_daemon::single_instance::acquire;
use fileuc_hook::gesture::{GestureDetector, GestureEvent, Modifiers};
use fileuc_hook::mouse_hook::{install, HookEvent};
use fileuc_hud::donut_geometry::{DonutLayout, HitResult};
use fileuc_hud::drop_target::DropTargetEvent;
use fileuc_hud::hud_window::HudWindow;
use fileuc_hud::progress_ring::ProgressTracker;
use fileuc_hud::renderer::{render_donut, render_progress, DonutState};
use fileuc_hud::smart_filter::filter_actions;
use fileuc_hud::theme::Theme;
use fileuc_settings::store::SettingsStore;
use fileuc_types::ConversionAction;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, PostQuitMessage, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
};

fn main() {
    let mutex_name = "FileUC_SingleInstance_Mutex";
    let pipe_name = "FileUC_Ipc_Pipe";

    let _guard = match acquire(mutex_name) {
        Ok(guard) => guard,
        Err(_) => {
            // Another instance is already running! Signal it to open settings and exit.
            let _ = send_message(pipe_name, IpcMessage::OpenSettings);
            return;
        }
    };

    unsafe {
        let _ = OleInitialize(None);
    }

    // Load persistent settings
    let settings_path = SettingsStore::default_path();
    let settings_store = Arc::new(SettingsStore::new(settings_path));
    let current_settings = Arc::new(Mutex::new(settings_store.load()));

    // Apply auto-start state
    {
        let auto_start = current_settings.lock().unwrap().auto_start;
        let _ = set_auto_start(auto_start);
    }

    // Start named pipe IPC server
    let store_for_ipc = settings_store.clone();
    let settings_for_ipc = current_settings.clone();
    let is_settings_open = Arc::new(AtomicBool::new(false));
    let open_flag = is_settings_open.clone();

    let _server = start_server(pipe_name, move |msg| match msg {
        IpcMessage::OpenSettings => {
            if !open_flag.swap(true, Ordering::SeqCst) {
                let st = settings_for_ipc.lock().unwrap().clone();
                let store = store_for_ipc.clone();
                let settings_ref = settings_for_ipc.clone();
                let flag = open_flag.clone();
                std::thread::spawn(move || {
                    if let Some(updated) = fileuc_settings::open_settings_window(st) {
                        let _ = store.save(&updated);
                        let _ = set_auto_start(updated.auto_start);
                        *settings_ref.lock().unwrap() = updated;
                    }
                    flag.store(false, Ordering::SeqCst);
                });
            }
        }
        IpcMessage::Quit => unsafe {
            PostQuitMessage(0);
        },
    })
    .expect("Failed to start IPC server");

    // Create Mouse Hook
    let (hook_tx, hook_rx) = mpsc::channel();
    let _hook = install(hook_tx).expect("Failed to install mouse hook");

    // Create Unified HUD Window & Drop Target
    let (drop_tx, drop_rx) = mpsc::channel();
    let mut hud = HudWindow::new(drop_tx).expect("Failed to create HudWindow");

    // State machine
    let mut detector = GestureDetector::new(5, Modifiers::CTRL);
    let mut donut_state: Option<DonutState> = None;
    let mut current_actions: Vec<ConversionAction> = Vec::new();
    let mut current_files: Vec<PathBuf> = Vec::new();
    let mut progress_tracker: Option<ProgressTracker> = None;
    let mut conversion_in_flight: Option<(JoinHandle<ConversionResult>, Arc<AtomicU32>)> = None;
    let mut hud_center = (0i32, 0i32);

    let mut pixmap = tiny_skia::Pixmap::new(300, 300).unwrap();
    let mut should_quit = false;

    while !should_quit {
        // 1. Process Windows Messages
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    should_quit = true;
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 2. Process Hook Events
        while let Ok(event) = hook_rx.try_recv() {
            let gesture = match event {
                HookEvent::MouseDown { btn, x, y, mods } => detector.mouse_down(btn, x, y, mods),
                HookEvent::MouseMove { x, y, mods } => detector.mouse_move(x, y, mods),
                HookEvent::MouseUp { btn, x, y, mods } => detector.mouse_up(btn, x, y, mods),
            };

            if let Some(g) = gesture {
                match g {
                    GestureEvent::DragStarted { origin_x, origin_y } => {
                        hud_center = (origin_x, origin_y);
                        hud.position_at_center(origin_x, origin_y);
                        hud.show();
                    }
                    GestureEvent::DragEnded => {
                        if progress_tracker.is_none() {
                            hud.hide();
                            donut_state = None;
                        }
                    }
                }
            }
        }

        // 3. Process HUD Drop Events
        while let Ok(event) = drop_rx.try_recv() {
            match event {
                DropTargetEvent::FilesEntered(files) => {
                    current_files = files.clone();
                    let has_ffmpeg = fileuc_convert::media::find_ffmpeg().is_some();
                    let actions = filter_actions(&files, has_ffmpeg);
                    current_actions = actions.clone();
                    let state = DonutState::new(actions, files.len());
                    donut_state = Some(state);
                }
                DropTargetEvent::DragOver { x, y } => {
                    if let Some(ref mut state) = donut_state {
                        let rel_x = (x - hud_center.0) as f32;
                        let rel_y = (y - hud_center.1) as f32;
                        let layout = DonutLayout::new(45.0, 120.0, current_actions.len());
                        let hit = layout.hit_test(rel_x, rel_y);
                        state.set_hovered(match hit {
                            HitResult::Slice(i) => Some(i),
                            _ => None,
                        });
                    }
                }
                DropTargetEvent::DragLeft | DropTargetEvent::NoFiles => {
                    if progress_tracker.is_none() {
                        hud.hide();
                        donut_state = None;
                    }
                }
                DropTargetEvent::Dropped { x, y } => {
                    let rel_x = (x - hud_center.0) as f32;
                    let rel_y = (y - hud_center.1) as f32;
                    let layout = DonutLayout::new(45.0, 120.0, current_actions.len());
                    let hit = layout.hit_test(rel_x, rel_y);

                    donut_state = None;

                    match hit {
                        HitResult::Slice(idx) if idx < current_actions.len() => {
                            let action = current_actions[idx];
                            let files = current_files.clone();
                            let settings = { current_settings.lock().unwrap().clone() };

                            let tracker = ProgressTracker::new();
                            progress_tracker = Some(tracker);

                            let progress_val = Arc::new(AtomicU32::new(0));
                            let p_clone = progress_val.clone();

                            let handle = dispatch(
                                ConversionRequest {
                                    files,
                                    action,
                                    settings: Arc::new(settings),
                                },
                                move |p| {
                                    let int_val = (p * 1000.0) as u32;
                                    p_clone.store(int_val, Ordering::Relaxed);
                                },
                            );

                            conversion_in_flight = Some((handle, progress_val));
                        }
                        _ => {
                            hud.hide();
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
                let res = h.join().unwrap();
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
        }

        // 5. Render HUD Frame
        if let Some(ref mut tracker) = progress_tracker {
            let theme = Theme::dark();
            render_progress(tracker, &theme, &mut pixmap);
            hud.repaint(&pixmap);
            if tracker.fade_elapsed() {
                progress_tracker = None;
                hud.hide();
            }
        } else if let Some(ref mut state) = donut_state {
            state.tick(1.0 / 60.0);
            let theme = Theme::dark();
            render_donut(state, &theme, &mut pixmap);
            hud.repaint(&pixmap);
        }

        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
