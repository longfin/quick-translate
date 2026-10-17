//! Linux Wayland: plain Wayland gives clipboard events only to the focused client, so watching
//! needs a data-control protocol, the one `wl-paste --watch` and clipboard managers use:
//! `ext-data-control-v1` (standardised, newer wlroots/KDE) or `wlr-data-control-unstable-v1`
//! (sway and other wlroots compositors, KDE Plasma). GNOME/Mutter implements neither; there
//! `start` fails and the caller falls back to XWayland or reports `Backend::Unsupported`.
//!
//! The two protocols are identical apart from their names, so one macro instantiates both.
//! The current selection's offer is kept so the text can be read from it on a trigger.

use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::{Mutex, OnceLock};

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::{Kind, Sink};
use crate::log;

/// Preferred first.
const TEXT_MIMES: [&str; 5] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"];

static CONN: OnceLock<Connection> = OnceLock::new();
static CURRENT: Mutex<Option<Box<dyn DataOffer>>> = Mutex::new(None);

/// What we need from an `ext_`/`zwlr_data_control_offer_v1`.
trait DataOffer: Send {
    fn mimes(&self) -> Vec<String>;
    fn receive_into(&self, mime: String, fd: BorrowedFd<'_>);
    fn discard(&self);
}

struct State {
    sink: Sink,
    /// False during the first roundtrip, which reports the selection that existed at startup.
    ready: bool,
}

impl State {
    fn selection(&mut self, offer: Option<Box<dyn DataOffer>>) {
        let kind = offer.as_ref().map(|o| {
            if o.mimes().iter().any(|m| TEXT_MIMES.contains(&m.as_str())) {
                Kind::Text
            } else {
                Kind::NonText
            }
        });
        let previous = std::mem::replace(&mut *CURRENT.lock().unwrap_or_else(|e| e.into_inner()), offer);
        if let Some(previous) = previous {
            previous.discard();
        }
        // `None` = the clipboard was cleared (owner quit), not a copy.
        if let (true, Some(kind)) = (self.ready, kind) {
            (self.sink)(kind);
        }
    }
}

macro_rules! data_control {
    ($name:ident, $($proto:ident)::+, $mgr:ident::$Mgr:ident, $dev:ident::$Dev:ident, $off:ident::$Off:ident) => {
        mod $name {
            use std::os::fd::BorrowedFd;
            use std::sync::Mutex;

            use wayland_client::globals::GlobalList;
            use wayland_client::protocol::wl_seat::WlSeat;
            use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
            use $($proto)::+::{$mgr::$Mgr, $dev::{self, $Dev}, $off::{self, $Off}};

            use super::{DataOffer, State};

            type Mimes = Mutex<Vec<String>>;

            pub fn bind(globals: &GlobalList, qh: &QueueHandle<State>, seat: &WlSeat) -> bool {
                let Ok(manager) = globals.bind::<$Mgr, _, _>(qh, 1..=2, ()) else { return false };
                manager.get_data_device(seat, qh, ());
                true
            }

            impl DataOffer for $Off {
                fn mimes(&self) -> Vec<String> {
                    let mimes = self.data::<Mimes>();
                    mimes.map(|m| m.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap_or_default()
                }
                fn receive_into(&self, mime: String, fd: BorrowedFd<'_>) {
                    self.receive(mime, fd);
                }
                fn discard(&self) {
                    self.destroy();
                }
            }

            impl Dispatch<$Mgr, ()> for State {
                fn event(_: &mut State, _: &$Mgr, _: <$Mgr as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<State>) {}
            }

            impl Dispatch<$Dev, ()> for State {
                fn event(state: &mut State, _: &$Dev, event: $dev::Event, _: &(), _: &Connection, _: &QueueHandle<State>) {
                    match event {
                        $dev::Event::Selection { id } => {
                            state.selection(id.map(|o| Box::new(o) as Box<dyn DataOffer>))
                        }
                        // We only watch CLIPBOARD; drop primary-selection offers right away.
                        $dev::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
                        $dev::Event::Finished => {
                            crate::log::write("trigger: wayland data-control device finished");
                        }
                        _ => {}
                    }
                }

                event_created_child!(State, $Dev, [
                    $dev::EVT_DATA_OFFER_OPCODE => ($Off, Mimes::default()),
                ]);
            }

            impl Dispatch<$Off, Mimes> for State {
                fn event(_: &mut State, _: &$Off, event: $off::Event, mimes: &Mimes, _: &Connection, _: &QueueHandle<State>) {
                    if let $off::Event::Offer { mime_type } = event {
                        mimes.lock().unwrap_or_else(|e| e.into_inner()).push(mime_type);
                    }
                }
            }
        }
    };
}

data_control!(ext, wayland_protocols::ext::data_control::v1::client,
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1);
data_control!(wlr, wayland_protocols_wlr::data_control::v1::client,
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1);

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(_: &mut State, _: &WlRegistry, _: <WlRegistry as wayland_client::Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<State>) {}
}

impl Dispatch<WlSeat, ()> for State {
    fn event(_: &mut State, _: &WlSeat, _: <WlSeat as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<State>) {}
}

pub fn start(sink: Sink) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    // Data-control devices are per seat; the first seat is the one with the keyboard in practice.
    let seat: WlSeat = globals.bind(&qh, 1..=1, ()).map_err(|_| "no wl_seat".to_string())?;
    let protocol = if ext::bind(&globals, &qh, &seat) {
        "ext-data-control-v1"
    } else if wlr::bind(&globals, &qh, &seat) {
        "wlr-data-control-unstable-v1"
    } else {
        return Err("compositor has neither ext-data-control nor wlr-data-control".into());
    };

    let mut state = State { sink, ready: false };
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
    state.ready = true;
    let _ = CONN.set(conn);
    log::write(format!("trigger: wayland clipboard via {protocol}"));

    std::thread::Builder::new()
        .name("clipboard-watch".into())
        .spawn(move || loop {
            if let Err(e) = queue.blocking_dispatch(&mut state) {
                log::write(format!("trigger: wayland clipboard watcher stopped: {e}"));
                break;
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read the current selection as text through data-control (works without focus, unlike
/// `wl_data_device`). Blocks until the source app has written it.
pub fn read_text() -> Option<String> {
    let conn = CONN.get()?;
    let (mut reader, writer) = std::io::pipe().ok()?;
    {
        let current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        let offer = current.as_ref()?;
        let mimes = offer.mimes();
        let mime = TEXT_MIMES.iter().find(|m| mimes.iter().any(|x| x == *m))?;
        offer.receive_into(mime.to_string(), writer.as_fd());
    }
    conn.flush().ok()?;
    drop(writer); // our copy of the write end, or read_to_end never sees EOF
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}
