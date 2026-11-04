//! Win32 calls stay on the Tauri event-loop thread. No global hooks, injected
//! input, focus-stealing workarounds or changes to process DPI awareness.
use std::{io, mem::size_of};
use traygent_core::popup::{place, DismissAction, Rect};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::{
        HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
        WindowsAndMessaging::{
            GetAncestor, GetCursorPos, GetForegroundWindow, GetWindowRect, IsChild, SetWindowPos,
            GA_ROOTOWNER, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER,
        },
    },
};

pub struct Placement {
    pub work: Rect,
    pub scale: f64,
    pub frame: Rect,
    pub logical_size: (f64, f64),
}

fn rect(value: RECT) -> Rect {
    Rect {
        x: value.left,
        y: value.top,
        width: (value.right - value.left).max(1) as u32,
        height: (value.bottom - value.top).max(1) as u32,
    }
}

fn native_rect(value: Rect) -> RECT {
    RECT {
        left: value.x,
        top: value.y,
        right: value.right() as i32,
        bottom: value.bottom() as i32,
    }
}

fn window_rect(hwnd: HWND) -> io::Result<Rect> {
    let mut value = RECT::default();
    // SAFETY: caller supplies a live window; output points to an initialized RECT.
    if unsafe { GetWindowRect(hwnd, &mut value) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(rect(value))
}

fn set_frame(hwnd: HWND, frame: Rect) -> io::Result<()> {
    // SAFETY: frame is bounded by the monitor work area; no retained pointers.
    let ok = unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            frame.x,
            frame.y,
            frame.width as i32,
            frame.height as i32,
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn position(
    hwnd: HWND,
    icon: Option<Rect>,
    preferred: Option<(f64, f64)>,
) -> io::Result<Placement> {
    let previous = window_rect(hwnd)?;
    // SAFETY: hwnd belongs to the live main window. Tauri uses per-monitor DPI.
    let old_scale = f64::from(unsafe { GetDpiForWindow(hwnd) }.max(96)) / 96.0;
    let logical_size = preferred.unwrap_or((
        f64::from(previous.width) / old_scale,
        f64::from(previous.height) / old_scale,
    ));
    let anchor = native_rect(icon.unwrap_or(previous));
    // SAFETY: valid input/output structures, sized according to the Win32 ABI.
    let monitor = unsafe { MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let bounds = rect(info.rcMonitor);
    let work = rect(info.rcWork);
    // No stale anchor is needed on startup before Explorer exposes the icon.
    let icon = icon.unwrap_or(Rect {
        x: work.right() as i32 - 1,
        y: work.bottom() as i32,
        width: 1,
        height: 1,
    });
    let mut dpi_x = 96;
    let mut dpi_y = 96;
    // SAFETY: monitor is from MonitorFromRect; both outputs are valid pointers.
    let result = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let mut scale = if result >= 0 {
        f64::from(dpi_x.max(96)) / 96.0
    } else {
        old_scale
    };
    // Moving across monitors synchronously delivers WM_DPICHANGED. A second pass
    // uses the destination window DPI and repairs the OS-suggested frame, avoiding
    // drift and old-monitor scaling when the panel is reopened elsewhere.
    for _ in 0..2 {
        let size = (
            (logical_size.0 * scale).round() as u32,
            (logical_size.1 * scale).round() as u32,
        );
        set_frame(
            hwnd,
            place(icon, bounds, work, size, (6.0 * scale).round() as u32),
        )?;
        scale = f64::from(unsafe { GetDpiForWindow(hwnd) }.max(96)) / 96.0;
    }
    Ok(Placement {
        work,
        scale,
        frame: window_rect(hwnd)?,
        logical_size,
    })
}

fn belongs_to_popup(hwnd: HWND, foreground: HWND) -> bool {
    // SAFETY: Win32 validates handles; no pointers are dereferenced here.
    unsafe {
        foreground == hwnd
            || IsChild(hwnd, foreground) != 0
            || GetAncestor(foreground, GA_ROOTOWNER) == hwnd
    }
}

/// Defer owned native UI, transient focus and in-progress tray presses.
pub fn dismiss_action(hwnd: HWND, icon: Option<Rect>) -> DismissAction {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == hwnd {
        return DismissAction::Focused;
    }
    if foreground.is_null() || belongs_to_popup(hwnd, foreground) {
        return DismissAction::Defer;
    }
    let mut cursor = POINT::default();
    if unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON)) } < 0
        && unsafe { GetCursorPos(&mut cursor) } != 0
        && icon.is_some_and(|rect| rect.contains(cursor.x, cursor.y))
    {
        return DismissAction::Defer;
    }
    DismissAction::Hide
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::{
        w,
        Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_CHILD, WS_POPUP},
    };

    struct NativeWindow(HWND);
    impl Drop for NativeWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }
    fn window(owner: HWND, style: u32) -> NativeWindow {
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Traygent popup test"),
                style,
                0,
                0,
                480,
                760,
                owner,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null(), "{}", io::Error::last_os_error());
        NativeWindow(hwnd)
    }

    #[test]
    fn native_owned_dialogs_and_webview_children_belong_to_popup() {
        let main = window(std::ptr::null_mut(), WS_POPUP);
        let dialog = window(main.0, WS_POPUP);
        let child = window(main.0, WS_CHILD);
        let unrelated = window(std::ptr::null_mut(), WS_POPUP);
        assert!(belongs_to_popup(main.0, main.0));
        assert!(belongs_to_popup(main.0, dialog.0));
        assert!(belongs_to_popup(main.0, child.0));
        assert!(!belongs_to_popup(main.0, unrelated.0));
    }

    #[test]
    fn native_frame_fits_actual_monitor_workarea_on_repeated_open() {
        // Real Win32 HWND/monitor/DPI calls, but not a simulated Explorer click.
        // Interactive taskbar/overflow/mixed-DPI validation is a separate check.
        let main = window(std::ptr::null_mut(), WS_POPUP);
        let first = position(main.0, None, None).unwrap();
        let icon = Rect {
            x: first.work.right() as i32 - 30,
            y: first.work.bottom() as i32 + 4,
            width: 24,
            height: 24,
        };
        let mut prior = None;
        for _ in 0..5 {
            let result = position(main.0, Some(icon), Some(first.logical_size)).unwrap();
            let frame = window_rect(main.0).unwrap();
            assert!(frame.x >= result.work.x && frame.y >= result.work.y);
            assert!(frame.right() <= result.work.right() && frame.bottom() <= result.work.bottom());
            if let Some(previous) = prior {
                assert_eq!(frame, previous);
            }
            prior = Some(frame);
        }
    }
}
