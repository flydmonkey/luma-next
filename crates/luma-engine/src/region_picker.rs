use std::{ffi::c_void, mem::size_of, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject,
        HGDIOBJ, InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, Rectangle, SelectObject,
        SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE},
        WindowsAndMessaging::{
            CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
            DispatchMessageW, GWLP_USERDATA, GetMessageW, GetWindowLongPtrW, IDC_CROSS, LWA_ALPHA,
            LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SW_SHOW, SetForegroundWindow,
            SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow, TranslateMessage, WM_CREATE,
            WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE,
            WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

#[derive(Clone, Copy, Debug)]
pub struct PickBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

struct PickerState {
    origin_x: i32,
    origin_y: i32,
    dragging: bool,
    start: POINT,
    current: POINT,
    result: Option<PickBounds>,
    cancelled: bool,
}

pub fn pick(bounds: PickBounds) -> Result<Option<PickBounds>, String> {
    if bounds.width < 32 || bounds.height < 32 {
        return Err("selected display is too small for region picking".into());
    }
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let class_name = wide("LumaNext.RegionPicker.v1");
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_CROSS) },
        hbrBackground: null_mut(),
        lpszClassName: class_name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassW(&class) };
    let state = Box::new(PickerState {
        origin_x: bounds.x,
        origin_y: bounds.y,
        dragging: false,
        start: POINT { x: 0, y: 0 },
        current: POINT { x: 0, y: 0 },
        result: None,
        cancelled: false,
    });
    let state_ptr = Box::into_raw(state);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name.as_ptr(),
            wide("选择录制区域 · 拖拽确认 · Esc 取消").as_ptr(),
            WS_POPUP,
            bounds.x,
            bounds.y,
            bounds.width as i32,
            bounds.height as i32,
            null_mut(),
            null_mut(),
            instance,
            state_ptr.cast::<c_void>(),
        )
    };
    if hwnd.is_null() {
        unsafe { drop(Box::from_raw(state_ptr)) };
        return Err("failed to create the native region picker window; an interactive desktop session is required".into());
    }
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, 190, LWA_ALPHA);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    let state = unsafe { Box::from_raw(state_ptr) };
    if state.cancelled {
        Ok(None)
    } else {
        Ok(state.result)
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = lparam as *const CREATESTRUCTW;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize) };
    }
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PickerState };
    if state_ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let state = unsafe { &mut *state_ptr };
    match message {
        WM_CREATE => 0,
        WM_LBUTTONDOWN => {
            state.dragging = true;
            state.start = point_from_lparam(lparam);
            state.current = state.start;
            unsafe { SetCapture(hwnd) };
            0
        }
        WM_MOUSEMOVE if state.dragging => {
            state.current = point_from_lparam(lparam);
            unsafe { InvalidateRect(hwnd, null_mut(), 0) };
            0
        }
        WM_LBUTTONUP if state.dragging => {
            state.dragging = false;
            state.current = point_from_lparam(lparam);
            unsafe { ReleaseCapture() };
            let rect = normalized_rect(state.start, state.current);
            if rect.right - rect.left >= 32 && rect.bottom - rect.top >= 32 {
                state.result = Some(PickBounds {
                    x: state.origin_x + rect.left,
                    y: state.origin_y + rect.top,
                    width: ((rect.right - rect.left) as u32) & !1,
                    height: ((rect.bottom - rect.top) as u32) & !1,
                });
                unsafe { DestroyWindow(hwnd) };
            } else {
                unsafe { InvalidateRect(hwnd, null_mut(), 0) };
            }
            0
        }
        WM_KEYDOWN if wparam as u16 == VK_ESCAPE => {
            state.cancelled = true;
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_PAINT => {
            paint(hwnd, state);
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn point_from_lparam(value: LPARAM) -> POINT {
    POINT {
        x: (value as u32 & 0xffff) as i16 as i32,
        y: ((value as u32 >> 16) & 0xffff) as i16 as i32,
    }
}

fn normalized_rect(a: POINT, b: POINT) -> RECT {
    RECT {
        left: a.x.min(b.x),
        top: a.y.min(b.y),
        right: a.x.max(b.x),
        bottom: a.y.max(b.y),
    }
}

fn paint(hwnd: HWND, state: &PickerState) {
    unsafe {
        let mut paint: PAINTSTRUCT = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut paint);
        let background = CreateSolidBrush(rgb(18, 18, 20));
        FillRect(dc, &paint.rcPaint, background);
        DeleteObject(background as HGDIOBJ);
        let hint = wide("拖拽选择录制区域 · 松开确认 · Esc 取消");
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, rgb(255, 255, 255));
        TextOutW(dc, 24, 24, hint.as_ptr(), (hint.len() - 1) as i32);
        if state.dragging {
            let rect = normalized_rect(state.start, state.current);
            let fill = CreateSolidBrush(rgb(230, 230, 232));
            FillRect(dc, &rect, fill);
            DeleteObject(fill as HGDIOBJ);
            let pen = CreatePen(PS_SOLID, 3, rgb(196, 43, 28));
            let old_pen = SelectObject(dc, pen as HGDIOBJ);
            let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
            Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            DeleteObject(pen as HGDIOBJ);
            let label = wide(&format!(
                "{} × {}",
                rect.right - rect.left,
                rect.bottom - rect.top
            ));
            SetTextColor(dc, rgb(196, 43, 28));
            TextOutW(
                dc,
                rect.left + 8,
                (rect.top - 24).max(8),
                label.as_ptr(),
                (label.len() - 1) as i32,
            );
        }
        EndPaint(hwnd, &paint);
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

const _: () = assert!(size_of::<isize>() == size_of::<*mut c_void>());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_is_normalized_in_any_drag_direction() {
        let rect = normalized_rect(POINT { x: 80, y: 60 }, POINT { x: 10, y: 20 });
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (10, 20, 80, 60)
        );
    }
}
