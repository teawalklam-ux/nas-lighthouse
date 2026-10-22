//! Remove the overlay's native caption at its source, without changing WebView alpha.
use core::ffi::c_void;
use tauri::WebviewWindow;

type Hwnd = *mut c_void;
type SubclassProc = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;
const SUBCLASS_ID: usize = 0x4A4F4F46;
const GWL_STYLE: i32 = -16;
// WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX
const FRAME_STYLES: u32 = 0x00CF0000;

#[repr(C)]
struct StyleStruct {
    old: u32,
    new: u32,
}

#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(hwnd: Hwnd, callback: SubclassProc, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Hwnd, callback: SubclassProc, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowLongW(hwnd: Hwnd, index: i32) -> i32;
    fn SetWindowLongW(hwnd: Hwnd, index: i32, value: i32) -> i32;
    fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
}

unsafe extern "system" fn frame_proc(
    hwnd: Hwnd,
    msg: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    _data: usize,
) -> isize {
    match msg {
        // Tao recomputes WS_CAPTION/WS_SYSMENU even for decorations(false).
        // Filter the proposed style before Windows can paint a frame, not later
        // on a focus event (which would permit a visible flash).
        0x007C if wparam as i32 == GWL_STYLE && lparam != 0 => {
            let _ = DefSubclassProc(hwnd, msg, wparam, lparam);
            (*(lparam as *mut StyleStruct)).new &= !FRAME_STYLES;
            0
        }
        // WM_NCPAINT: this window has no native non-client surface to draw.
        0x0085 => 0,
        // WM_NCACTIVATE: keep Tao's focus bookkeeping, but tell DefWindowProc
        // not to repaint the non-client area (documented lParam = -1).
        0x0086 => DefSubclassProc(hwnd, msg, wparam, -1),
        // WM_NCDESTROY: do not leave a callback attached to a destroyed HWND.
        0x0082 => {
            let _ = RemoveWindowSubclass(hwnd, frame_proc, id);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

pub fn install(win: &WebviewWindow) -> tauri::Result<()> {
    let window = win.clone();
    // SetWindowSubclass must execute on the thread that owns the HWND.
    win.run_on_main_thread(move || {
        let Ok(hwnd) = window.hwnd() else { return };
        unsafe {
            if SetWindowSubclass(hwnd.0, frame_proc, SUBCLASS_ID, 0) == 0 {
                eprintln!("overlay: could not install native caption suppression");
                return;
            }
            let style = GetWindowLongW(hwnd.0, GWL_STYLE) as u32;
            if style & FRAME_STYLES != 0 {
                SetWindowLongW(hwnd.0, GWL_STYLE, (style & !FRAME_STYLES) as i32);
                // Refresh only the frame cache; retain size, position, z-order,
                // activation, transparency and the existing input region.
                let _ = SetWindowPos(hwnd.0, core::ptr::null_mut(), 0, 0, 0, 0, 0x0037);
            }
        }
    })
}
