use std::mem::size_of;

use windows::core::Result;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowPlacement,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE,
    GW_OWNER, HWND_TOP, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE,
    WINDOWPLACEMENT, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_SYSMENU, WS_THICKFRAME,
};

/// A window we can offer the user to fullscreen.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub process_name: String,
}

/// Everything we need to put a window back exactly how we found it.
#[derive(Debug, Clone, Copy)]
pub struct SavedState {
    style: isize,
    extended_style: isize,
    placement: WINDOWPLACEMENT,
}

/// Enumerate visible, titled, top-level windows (skipping tool windows,
/// owned windows, and windows with no title).
pub fn list_windows() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Vec::new();

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
        );
    }

    windows
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip child windows
    if GetWindow(hwnd, GW_OWNER).unwrap_or_default() != HWND::default() {
        return BOOL(1);
    }

    let extended_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    let is_tool_window = (extended_style & WS_EX_TOOLWINDOW.0) != 0;
    let is_app_window = (extended_style & WS_EX_APPWINDOW.0) != 0;
    if is_tool_window && !is_app_window {
        return BOOL(1);
    }

    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return BOOL(1);
    }

    let mut class_buf = [0u16; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf) as usize;
    let class_name = String::from_utf16_lossy(&class_buf[..class_len]);
    if matches!(
        class_name.as_str(),
        "Progman" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "WorkerW"
    ) {
        return BOOL(1);
    }

    let mut title_buf = vec![0u16; title_len as usize + 1];
    let written = GetWindowTextW(hwnd, &mut title_buf);
    if written == 0 {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(&title_buf[..written as usize]);

    let process_name = get_process_name(hwnd).unwrap_or_else(|| "?".to_string());

    windows.push(WindowInfo {
        hwnd,
        title,
        process_name,
    });

    BOOL(1)
}

fn get_process_name(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(process);

        if ok.is_err() {
            return None;
        }

        let full_path = String::from_utf16_lossy(&buf[..len as usize]);
        full_path.rsplit(['\\', '/']).next().map(|s| s.to_string())
    }
}

/// Strip a window's border and stretch it to fill its monitor.
/// Returns the previous style and position so it can be restored later.
pub fn fullscreen_window(hwnd: HWND) -> Result<SavedState> {
    unsafe {
        // Un-minimize first; SetWindowPos on a minimized window is a no-op
        // for the visible frame.
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let extended_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
        placement.length = size_of::<WINDOWPLACEMENT>() as u32;
        GetWindowPlacement(hwnd, &mut placement)?;
        let saved = SavedState {
            style,
            extended_style,
            placement,
        };

        let monitor: HMONITOR = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return Err(windows::core::Error::from_win32());
        }

        let remove_bits =
            (WS_CAPTION.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0)
                as isize;
        let new_style = style & !remove_bits;
        SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);

        let mr = mi.rcMonitor;
        SetWindowPos(
            hwnd,
            HWND_TOP,
            mr.left,
            mr.top,
            mr.right - mr.left,
            mr.bottom - mr.top,
            SWP_NOZORDER | SWP_FRAMECHANGED,
        )?;

        Ok(saved)
    }
}

/// Put a window's style and position back to what `fullscreen_window` saved.
pub fn restore_window(hwnd: HWND, saved: &SavedState) -> Result<()> {
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, saved.style);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, saved.extended_style);
        // Force frame recalculation before restoring placement.
        SetWindowPos(
            hwnd,
            HWND::default(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        )?;
        SetWindowPlacement(hwnd, &saved.placement)?;
        Ok(())
    }
}
