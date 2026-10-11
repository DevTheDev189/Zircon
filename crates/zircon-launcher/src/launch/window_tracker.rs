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
    #[allow(unused_variables)] pid: u32,
) {
    tauri::async_runtime::spawn(async move {
        let start_time = Instant::now();
        // Maximum time to wait for window initialization before falling back (75 seconds).
        let max_wait = Duration::from_secs(75);
        #[allow(unused_variables, unused_mut)]
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
    fn GetSystemMetrics(n_index: i32) -> i32;
    fn GetWindowLongW(h_wnd: HWND, n_index: i32) -> i32;
    fn SetWindowLongW(h_wnd: HWND, n_index: i32, dw_new_long: i32) -> i32;
    fn SetWindowPos(
        h_wnd: HWND,
        h_wnd_insert_after: HWND,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        u_flags: u32,
    ) -> BOOL;
    fn ShowWindow(h_wnd: HWND, n_cmd_show: i32) -> BOOL;
}

#[cfg(target_os = "windows")]
const GWL_STYLE: i32 = -16;
#[cfg(target_os = "windows")]
const WS_CAPTION: i32 = 0x00C00000;
#[cfg(target_os = "windows")]
const WS_THICKFRAME: i32 = 0x00040000;
#[cfg(target_os = "windows")]
const WS_MINIMIZEBOX: i32 = 0x00020000;
#[cfg(target_os = "windows")]
const WS_MAXIMIZEBOX: i32 = 0x00010000;
#[cfg(target_os = "windows")]
const WS_SYSMENU: i32 = 0x00080000;
#[cfg(target_os = "windows")]
const SWP_FRAMECHANGED: u32 = 0x0020;
#[cfg(target_os = "windows")]
const SWP_SHOWWINDOW: u32 = 0x0040;
#[cfg(target_os = "windows")]
const SW_RESTORE: i32 = 9;
#[cfg(target_os = "windows")]
const SM_CXSCREEN: i32 = 0;
#[cfg(target_os = "windows")]
const SM_CYSCREEN: i32 = 1;

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

#[cfg(target_os = "windows")]
pub fn get_primary_screen_resolution() -> (i32, i32) {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w > 0 && h > 0 {
            (w, h)
        } else {
            (1920, 1080)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_primary_screen_resolution() -> (i32, i32) {
    (1920, 1080)
}

#[cfg(target_os = "windows")]
pub fn snap_pid_window_borderless(pid: u32, x: i32, y: i32, width: i32, height: i32) -> bool {
    struct FindContext {
        target_pid: u32,
        hwnd: Option<HWND>,
    }
    unsafe extern "system" fn find_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam as *mut FindContext);
        let mut win_pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut win_pid);
        if win_pid == ctx.target_pid && IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;
                if w >= 200 && h >= 200 {
                    ctx.hwnd = Some(hwnd);
                    return 0;
                }
            }
        }
        1
    }
    let mut ctx = FindContext {
        target_pid: pid,
        hwnd: None,
    };
    unsafe {
        EnumWindows(find_cb, &mut ctx as *mut FindContext as LPARAM);
    }
    if let Some(hwnd) = ctx.hwnd {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let borderless =
                style & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU);
            SetWindowLongW(hwnd, GWL_STYLE, borderless);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_FRAMECHANGED | SWP_SHOWWINDOW,
            ) != 0
        }
    } else {
        false
    }
}

#[cfg(not(target_os = "windows"))]
pub fn snap_pid_window_borderless(_pid: u32, _x: i32, _y: i32, _width: i32, _height: i32) -> bool {
    false
}

pub fn spawn_splitscreen_snapper(rects: Vec<(u32, i32, i32, i32, i32)>) {
    tauri::async_runtime::spawn(async move {
        let start = Instant::now();
        let max_duration = Duration::from_secs(45);
        let mut snapped_counts = vec![0usize; rects.len()];

        while start.elapsed() < max_duration {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let mut all_done = true;

            for (idx, (pid, x, y, w, h)) in rects.iter().enumerate() {
                if snapped_counts[idx] < 3 {
                    all_done = false;
                    let success = snap_pid_window_borderless(*pid, *x, *y, *w, *h);
                    if success {
                        snapped_counts[idx] += 1;
                        tracing::debug!(
                            "Snapped PID {pid} to ({x}, {y}, {w}, {h}) [attempt {}/3]",
                            snapped_counts[idx]
                        );
                    }
                }
            }

            if all_done {
                tracing::info!("All split-screen windows successfully positioned and stabilized.");
                break;
            }
        }
    });
}
