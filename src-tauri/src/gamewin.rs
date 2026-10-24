use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

fn saved_file() -> std::path::PathBuf {
    crate::paths::root().join("window.json")
}

fn load_saved() -> Option<Rect> {
    serde_json::from_str(&std::fs::read_to_string(saved_file()).ok()?).ok()
}

fn store_saved(rect: Rect) {
    if let Ok(json) = serde_json::to_string(&rect) {
        let _ = crate::paths::write_atomic(&saved_file(), json.as_bytes());
    }
}

pub fn center_in(area: Rect, w: i32, h: i32) -> Rect {
    let w = w.min(area.w);
    let h = h.min(area.h);
    Rect { x: area.x + (area.w - w) / 2, y: area.y + (area.h - h) / 2, w, h }
}

pub fn visible_on(rect: Rect, screens: &[Rect]) -> bool {
    screens.iter().any(|s| rect.x + 40 < s.x + s.w && rect.x + rect.w - 40 > s.x && rect.y >= s.y - 10 && rect.y + 40 < s.y + s.h)
}

#[cfg(windows)]
mod win {
    use super::Rect;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    pub struct Screen {
        pub name: String,
        pub full: Rect,
        pub work: Rect,
    }

    fn rect(r: &RECT) -> Rect {
        Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top }
    }

    fn info(monitor: HMONITOR) -> Option<Screen> {
        let mut mi: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO) } == 0 {
            return None;
        }
        let len = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
        Some(Screen { name: String::from_utf16_lossy(&mi.szDevice[..len]), full: rect(&mi.monitorInfo.rcMonitor), work: rect(&mi.monitorInfo.rcWork) })
    }

    unsafe extern "system" fn collect_monitor(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let list = unsafe { &mut *(data as *mut Vec<Screen>) };
        if let Some(screen) = info(monitor) {
            list.push(screen);
        }
        1
    }

    pub fn screens() -> Vec<Screen> {
        let mut list: Vec<Screen> = Vec::new();
        unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(collect_monitor), &mut list as *mut Vec<Screen> as LPARAM) };
        list
    }

    struct Search {
        pid: u32,
        found: HWND,
    }

    unsafe extern "system" fn match_window(hwnd: HWND, data: LPARAM) -> BOOL {
        let search = unsafe { &mut *(data as *mut Search) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == search.pid && unsafe { IsWindowVisible(hwnd) } != 0 && unsafe { GetWindowTextLengthW(hwnd) } > 0 {
            search.found = hwnd;
            return 0;
        }
        1
    }

    pub fn find(pid: u32) -> Option<HWND> {
        let mut search = Search { pid, found: std::ptr::null_mut() };
        unsafe { EnumWindows(Some(match_window), &mut search as *mut Search as LPARAM) };
        (!search.found.is_null()).then_some(search.found)
    }

    pub fn alive(hwnd: HWND) -> bool {
        unsafe { IsWindow(hwnd) != 0 }
    }

    pub fn bounds(hwnd: HWND) -> Option<Rect> {
        let mut r: RECT = unsafe { std::mem::zeroed() };
        let normal = unsafe { IsIconic(hwnd) == 0 && IsZoomed(hwnd) == 0 };
        (normal && unsafe { GetWindowRect(hwnd, &mut r) } != 0).then(|| rect(&r))
    }

    pub fn current_screen(hwnd: HWND) -> Option<Screen> {
        info(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) })
    }

    pub fn place(hwnd: HWND, r: Rect, borderless: bool) {
        unsafe {
            if borderless {
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                let bare = (style & !(WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX)) | WS_POPUP;
                SetWindowLongPtrW(hwnd, GWL_STYLE, bare as isize);
            }
            SetWindowPos(hwnd, HWND_TOP, r.x, r.y, r.w, r.h, SWP_FRAMECHANGED | SWP_SHOWWINDOW);
        }
    }
}

#[cfg(windows)]
pub fn manage(pid: u32, settings: &crate::config::Settings) {
    let mode = settings.window_mode.clone();
    let monitor = settings.window_monitor.clone();
    let remember = settings.remember_window;
    if mode == "normal" && monitor.is_empty() && !remember {
        return;
    }
    std::thread::spawn(move || {
        let mut hwnd = None;
        for _ in 0..240 {
            hwnd = win::find(pid);
            if hwnd.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let Some(hwnd) = hwnd else { return };
        std::thread::sleep(std::time::Duration::from_millis(800));
        let screens = win::screens();
        let target = screens.iter().find(|s| !monitor.is_empty() && s.name == monitor).map(|s| (s.full, s.work)).or_else(|| win::current_screen(hwnd).map(|s| (s.full, s.work)));
        if mode == "borderless" {
            if let Some((full, _)) = target {
                win::place(hwnd, full, true);
            }
            return;
        }
        let saved = load_saved().filter(|r| visible_on(*r, &screens.iter().map(|s| s.work).collect::<Vec<_>>()));
        if let Some(r) = saved.filter(|_| remember) {
            win::place(hwnd, r, false);
        } else if let (Some((_, work)), Some(now)) = (target.filter(|_| !monitor.is_empty()), win::bounds(hwnd)) {
            win::place(hwnd, center_in(work, now.w, now.h), false);
        }
        if !remember {
            return;
        }
        let mut last = None;
        while win::alive(hwnd) {
            if let Some(r) = win::bounds(hwnd) {
                if last != Some(r) && r.w > 200 && r.h > 150 {
                    store_saved(r);
                    last = Some(r);
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

#[cfg(not(windows))]
pub fn manage(_pid: u32, _settings: &crate::config::Settings) {}

#[derive(Serialize)]
pub struct ScreenInfo {
    pub name: String,
    pub width: i32,
    pub height: i32,
}

#[tauri::command]
pub fn list_screens() -> Vec<ScreenInfo> {
    #[cfg(windows)]
    {
        win::screens().into_iter().map(|s| ScreenInfo { name: s.name, width: s.full.w, height: s.full.h }).collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[tauri::command]
pub fn forget_window() {
    let _ = std::fs::remove_file(saved_file());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    #[ignore]
    fn finds_real_screens_and_windows() {
        let screens = win::screens();
        assert!(!screens.is_empty());
        for s in &screens {
            println!("screen {} {:?}", s.name, s.full);
        }
        let mut child = std::process::Command::new("charmap").spawn().unwrap();
        let mut found = None;
        for _ in 0..40 {
            found = win::find(child.id());
            if found.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let hwnd = found.expect("找不到字元對應表視窗");
        let target = center_in(screens[0].work, 640, 400);
        win::place(hwnd, target, false);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let now = win::bounds(hwnd).unwrap();
        println!("placed {:?} -> {:?}", target, now);
        let _ = child.kill();
        assert!((now.x - target.x).abs() < 20 && (now.w - target.w).abs() < 20);
    }

    #[test]
    fn centers_and_checks_visibility() {
        let screen = Rect { x: 1920, y: 0, w: 2560, h: 1400 };
        assert_eq!(center_in(screen, 854, 480), Rect { x: 1920 + 853, y: 460, w: 854, h: 480 });
        assert_eq!(center_in(screen, 4000, 3000), screen);
        let screens = [Rect { x: 0, y: 0, w: 1920, h: 1040 }, screen];
        assert!(visible_on(Rect { x: 2000, y: 100, w: 800, h: 600 }, &screens));
        assert!(!visible_on(Rect { x: 5000, y: 100, w: 800, h: 600 }, &screens));
        assert!(!visible_on(Rect { x: 100, y: -500, w: 800, h: 400 }, &screens));
    }
}
