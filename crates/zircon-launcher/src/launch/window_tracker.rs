//! Background window tracker for Minecraft launch.
//!
//! Monitors the newly spawned Minecraft process (and child windows) on Windows
//! to detect when the game has completed initialization, created its display
//! window, and transitioned into fullscreen mode (or visible windowed mode).
//!
//! Emits the `game-window-ready` event when the game is ready and clears
//! `always_on_top` on the launcher window.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

/// Starts monitoring the Minecraft window in a background task.
pub fn spawn_window_tracker(
    app: AppHandle,
    game_id: u64,
    pid: u32,
) {
    tauri::async_runtime::spawn(async move {
        let start_time = Instant::now();
        // Maximum time to wait for window initialization before falling back (75 seconds).
        let max_wait = Duration::from_secs(75);
        let mut visible_streak = 0u32;

        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;

            let state = app.state::<crate::commands::LauncherState>();
            if state.launch_cancellation.is_aborted() {
                clear_always_on_top(&app);
                break;
            }
            {
                let guard = state.running_game.lock().await;
                if let Some(game) = guard.as_ref() {
                    if game.id != game_id {
                        clear_always_on_top(&app);
                        break;
                    }
                } else {
                    clear_always_on_top(&app);
                    break;
                }
            }

            if start_time.elapsed() > max_wait {
                tracing::info!("Window tracker reached max wait timeout — assuming game ready.");
                notify_window_ready(&app, game_id, false);
                break;
            }

            #[cfg(target_os = "windows")]
            {
                let status = check_windows_process_window(pid);
                if status.is_fullscreen {
                    tracing::info!(
                        "Minecraft process {} reached fullscreen after {:?}",
                        pid,
                        start_time.elapsed()
                    );
                    notify_window_ready(&app, game_id, true);
                    break;
                } else if status.is_visible {
                    visible_streak += 1;
                    // If the window has been continuously visible and sizable for >= 1.5 seconds,
                    // consider it ready even if not strictly borderless/exclusive fullscreen.
                    if visible_streak >= 6 {
                        tracing::info!(
                            "Minecraft process {} window is visible and active after {:?}",
                            pid,
                            start_time.elapsed()
                        );
                        notify_window_ready(&app, game_id, false);
                        break;
                    }
                } else {
                    visible_streak = 0;
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                // Fallback for non-Windows: after 5 seconds of process uptime, treat as ready.
                if start_time.elapsed() >= Duration::from_secs(5) {
                    notify_window_ready(&app, game_id, true);
                    break;
                }
            }
        }
    });
}

fn notify_window_ready(app: &AppHandle, game_id: u64, fullscreen: bool) {
    clear_always_on_top(app);
    let _ = app.emit(
        "game-window-ready",
        serde_json::json!({
            "gameId": game_id,
            "fullscreen": fullscreen,
        }),
    );
    let _ = app.emit("launch-status", "Minecraft is ready.");
}

pub fn clear_always_on_top(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(false);
    }
}

pub fn set_always_on_top(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(true);
        let _ = win.set_focus();
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessWindowStatus {
    pub is_visible: bool,
    pub is_fullscreen: bool,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct MONITORINFO {
    cb_size: u32,
    rc_monitor: RECT,
    rc_work: RECT,
    dw_flags: u32,
}

#[cfg(target_os = "windows")]
type HWND = *mut std::ffi::c_void;
#[cfg(target_os = "windows")]
type HMONITOR = *mut std::ffi::c_void;
#[cfg(target_os = "windows")]
type BOOL = i32;
#[cfg(target_os = "windows")]
type LPARAM = isize;

#[cfg(target_os = "windows")]
const MONITOR_DEFAULTTONEAREST: u32 = 2;

#[cfg(target_os = "windows")]
extern "system" {
    fn EnumWindows(
        lp_enum_func: unsafe extern "system" fn(HWND, LPARAM) -> BOOL,
        l_param: LPARAM,
    ) -> BOOL;
    fn GetWindowThreadProcessId(h_wnd: HWND, lpdw_process_id: *mut u32) -> u32;
    fn IsWindowVisible(h_wnd: HWND) -> BOOL;
    fn IsIconic(h_wnd: HWND) -> BOOL;
    fn GetWindowRect(h_wnd: HWND, lp_rect: *mut RECT) -> BOOL;
    fn MonitorFromWindow(h_wnd: HWND, dw_flags: u32) -> HMONITOR;
    fn GetMonitorInfoW(h_monitor: HMONITOR, lpmi: *mut MONITORINFO) -> BOOL;
}

#[cfg(target_os = "windows")]
struct EnumContext {
    target_pid: u32,
    status: ProcessWindowStatus,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam as *mut EnumContext);
    let mut win_pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut win_pid);

    if win_pid == ctx.target_pid {
        if IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                // Ignore tiny hidden/helper tool windows
                if width >= 200 && height >= 200 {
                    ctx.status.is_visible = true;

                    let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    if !hmon.is_null() {
                        let mut mi = MONITORINFO::default();
                        mi.cb_size = std::mem::size_of::<MONITORINFO>() as u32;
                        if GetMonitorInfoW(hmon, &mut mi) != 0 {
                            // Fullscreen means window rect covers or extends beyond monitor bounds
                            let covers_monitor = rect.left <= mi.rc_monitor.left
                                && rect.top <= mi.rc_monitor.top
                                && rect.right >= mi.rc_monitor.right
                                && rect.bottom >= mi.rc_monitor.bottom;

                            if covers_monitor {
                                ctx.status.is_fullscreen = true;
                                return 0; // Stop enumeration once fullscreen window is found
                            }
                        }
                    }
                }
            }
        }
    }
    1 // Continue enumeration
}

#[cfg(target_os = "windows")]
pub fn check_windows_process_window(pid: u32) -> ProcessWindowStatus {
    let mut ctx = EnumContext {
        target_pid: pid,
        status: ProcessWindowStatus::default(),
    };
    unsafe {
        EnumWindows(enum_window_callback, &mut ctx as *mut EnumContext as LPARAM);
    }
    ctx.status
}
