//! Windows: a message-only window registered with `AddClipboardFormatListener` receives
//! `WM_CLIPBOARDUPDATE` on its own thread. No polling, no permissions.
//!
//! `GetClipboardSequenceNumber` drops notifications that don't correspond to a new clipboard
//! (the listener can be re-notified for the same content), and `IsClipboardFormatAvailable`
//! classifies the change without opening the clipboard (which would block the writer): text,
//! or one of the image / file formats below, else unknown (counted like text).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Ole::{
    CF_BITMAP, CF_DIB, CF_DIBV5, CF_ENHMETAFILE, CF_HDROP, CF_UNICODETEXT, CLIPBOARD_FORMAT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE, WNDCLASSW,
};

use super::{Kind, Sink};

static SINK: OnceLock<Sink> = OnceLock::new();

/// Formats that mean "an image or files were copied" when no text format is offered.
const NON_TEXT: [CLIPBOARD_FORMAT; 5] = [CF_BITMAP, CF_DIB, CF_DIBV5, CF_ENHMETAFILE, CF_HDROP];

pub fn start(sink: Sink) -> Result<(), String> {
    SINK.set(sink).map_err(|_| "already started".to_string())?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("clipboard-watch".into())
        .spawn(move || {
            // The window must be created on the thread that pumps its messages.
            match unsafe { create_listener() } {
                Ok(()) => {
                    let _ = tx.send(Ok(()));
                    unsafe { pump() };
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            }
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn create_listener() -> Result<(), String> {
    let class_name = wide("QuickTranslateClipboardListener");
    let instance = GetModuleHandleW(std::ptr::null());
    let class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: instance,
        lpszClassName: class_name.as_ptr(),
        // SAFETY: all-zero is a valid WNDCLASSW for the remaining fields.
        ..std::mem::zeroed()
    };
    if RegisterClassW(&class) == 0 {
        return Err(format!("RegisterClassW failed ({})", std::io::Error::last_os_error()));
    }
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        class_name.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return Err(format!("CreateWindowExW failed ({})", std::io::Error::last_os_error()));
    }
    if AddClipboardFormatListener(hwnd) == 0 {
        return Err(format!(
            "AddClipboardFormatListener failed ({})",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

unsafe fn pump() {
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        on_update();
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn on_update() {
    static LAST_SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = unsafe { GetClipboardSequenceNumber() };
    if seq != 0 && LAST_SEQ.swap(seq, Ordering::Relaxed) == seq {
        return;
    }
    let kind = unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) != 0 {
            Kind::Text
        } else if NON_TEXT.iter().any(|&f| IsClipboardFormatAvailable(f as u32) != 0) {
            Kind::NonText
        } else {
            // Emptied, or a writer whose text isn't listed yet: OLE data objects (.NET's
            // Clipboard.SetText, Office) can show up without CF_UNICODETEXT at notification
            // time, so the read at trigger time decides (`read_text_with_retry`).
            Kind::Unknown
        }
    };
    if let Some(sink) = SINK.get() {
        sink(kind);
    }
}
