//! Linux X11 (and XWayland): XFixes `SelectSelectionInput` on CLIPBOARD delivers an event every
//! time any client takes ownership, i.e. on every Ctrl+C, even when the same app copies twice.
//! Each change is classified by asking the new owner for its TARGETS.
//!
//! Without XFixes (practically never: every X server since 2003 has it), poll the owner and its
//! `TIMESTAMP` target every 60 ms instead.

use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::errors::ReplyOrIdError;
use x11rb::protocol::xfixes::{self, ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE};

use super::{Kind, Sink};
use crate::log;

pub enum Mode {
    XFixes,
    Polling,
}

struct Atoms {
    clipboard: Atom,
    targets: Atom,
    timestamp: Atom,
    /// Property on our window that owners write their answers to.
    reply: Atom,
    text: [Atom; 5],
}

pub fn start(sink: Sink) -> Result<Mode, String> {
    let (conn, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let (window, atoms, mode) = setup(&conn, screen).map_err(|e| e.to_string())?;
    let polling = matches!(mode, Mode::Polling);
    std::thread::Builder::new()
        .name("clipboard-watch".into())
        .spawn(move || {
            if let Err(e) = run(&conn, window, &atoms, polling, &sink) {
                log::write(format!("trigger: X11 clipboard watcher stopped: {e}"));
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(mode)
}

fn setup(conn: &RustConnection, screen: usize) -> Result<(Window, Atoms, Mode), ReplyOrIdError> {
    let root = conn.setup().roots[screen].root;
    let window = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        window,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        COPY_FROM_PARENT,
        &CreateWindowAux::new(),
    )?;
    let intern = |name: &[u8]| -> Result<Atom, ReplyOrIdError> {
        Ok(conn.intern_atom(false, name)?.reply()?.atom)
    };
    let atoms = Atoms {
        clipboard: intern(b"CLIPBOARD")?,
        targets: intern(b"TARGETS")?,
        timestamp: intern(b"TIMESTAMP")?,
        reply: intern(b"QUICKTRANSLATE_SELECTION")?,
        text: [
            intern(b"UTF8_STRING")?,
            AtomEnum::STRING.into(),
            intern(b"TEXT")?,
            intern(b"text/plain;charset=utf-8")?,
            intern(b"text/plain")?,
        ],
    };

    let has_xfixes = conn.extension_information(xfixes::X11_EXTENSION_NAME)?.is_some();
    let mode = if has_xfixes {
        // The version handshake is mandatory before any other XFixes request.
        conn.xfixes_query_version(5, 0)?.reply()?;
        conn.xfixes_select_selection_input(
            window,
            atoms.clipboard,
            SelectionEventMask::SET_SELECTION_OWNER,
        )?;
        Mode::XFixes
    } else {
        log::write("trigger: X server has no XFixes, polling the clipboard owner");
        Mode::Polling
    };
    conn.flush()?;
    Ok((window, atoms, mode))
}

fn run(
    conn: &RustConnection,
    window: Window,
    atoms: &Atoms,
    polling: bool,
    sink: &Sink,
) -> Result<(), ReplyOrIdError> {
    // Polling: (owner, owner's TIMESTAMP) of the last poll; `None` until the first answer.
    let mut owner = NONE;
    let mut last: Option<(Window, Option<u32>)> = None;
    loop {
        let event = if polling {
            match conn.poll_for_event()? {
                Some(event) => event,
                None => {
                    std::thread::sleep(Duration::from_millis(60));
                    owner = conn.get_selection_owner(atoms.clipboard)?.reply()?.owner;
                    let target = atoms.timestamp;
                    conn.convert_selection(window, atoms.clipboard, target, atoms.reply, CURRENT_TIME)?;
                    conn.flush()?;
                    continue;
                }
            }
        } else {
            conn.wait_for_event()?
        };

        match event {
            Event::XfixesSelectionNotify(e) if e.owner != NONE => {
                // Ask the new owner what it offers; the answer arrives as SelectionNotify.
                let time = e.selection_timestamp;
                conn.convert_selection(window, atoms.clipboard, atoms.targets, atoms.reply, time)?;
                conn.flush()?;
            }
            Event::SelectionNotify(e) if e.requestor == window => {
                let values: Vec<u32> = if e.property == NONE {
                    Vec::new()
                } else {
                    let prop = conn.get_property(true, window, e.property, AtomEnum::ANY, 0, 4096)?;
                    prop.reply()?.value32().map(|v| v.collect()).unwrap_or_default()
                };
                if e.target == atoms.targets {
                    let kind = if values.is_empty() {
                        Kind::Unknown // owner refused or doesn't list TARGETS
                    } else if values.iter().any(|t| atoms.text.contains(t)) {
                        Kind::Text
                    } else {
                        Kind::NonText
                    };
                    sink(kind);
                } else if e.target == atoms.timestamp {
                    let now = (owner, values.first().copied());
                    if last.is_some_and(|prev| prev != now) && owner != NONE {
                        sink(Kind::Unknown);
                    }
                    last = Some(now);
                }
            }
            _ => {}
        }
    }
}
