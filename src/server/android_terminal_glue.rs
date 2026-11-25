//! Android glue for [`super::android_terminal_host`]: routes rustdesk `TerminalAction`s to VictorSuite's shell broker through JNI
//! (`scrap::android::call_main_service_victor_terminal` -> `MainService.rustVictorTerminal` -> `VictorHostTerminal`), and broker
//! events (`ffi.FFI.onVictorTerminalEvent`) back to the right rustdesk connection. No terminal bytes are logged.

use super::android_terminal_host::{Action, BrokerEvent, Effect, Registry, Response};
use super::service::Subscriber;
use super::ConnInner;
use base::message_proto::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref REGISTRY: Mutex<Registry> = Mutex::new(Registry::new(hbb_common::rand::random::<u64>()));
    static ref CONNS: Mutex<HashMap<i32, ConnInner>> = Mutex::new(HashMap::new());
}

/// True only when VictorSuite is installed, trusts rustdeskV for terminals, reports broker API >= 1 and has remote shells switched on.
pub fn broker_ready() -> bool {
    scrap::android::call_main_service_get_by_name("victorsuite_terminal_ready").map(|s| s == "true").unwrap_or(false)
}

pub fn register_conn(inner: &ConnInner) {
    CONNS.lock().unwrap().insert(inner.id(), inner.clone());
}

pub fn on_disconnect(conn_id: i32) {
    CONNS.lock().unwrap().remove(&conn_id);
    let effects = REGISTRY.lock().unwrap().on_disconnect(conn_id);
    run(conn_id, effects);
}

pub fn on_action(conn_id: i32, peer: &str, action: &TerminalAction) {
    let a = match &action.union {
        Some(terminal_action::Union::Open(o)) => Action::Open { terminal_id: o.terminal_id, rows: o.rows, cols: o.cols },
        Some(terminal_action::Union::Data(d)) => {
            if d.compressed {
                // Never decompress-and-guess here; a client that compresses input is not speaking the phone host's dialect.
                return;
            }
            Action::Data { terminal_id: d.terminal_id, data: d.data.to_vec() }
        }
        Some(terminal_action::Union::Resize(r)) => Action::Resize { terminal_id: r.terminal_id, rows: r.rows, cols: r.cols },
        Some(terminal_action::Union::Close(c)) => Action::Close { terminal_id: c.terminal_id },
        _ => return,
    };
    // Readiness is checked at every open (VictorSuite may have been switched off or untrusted since login).
    let ready = if matches!(a, Action::Open { .. }) { broker_ready() } else { true };
    let effects = REGISTRY.lock().unwrap().on_action(conn_id, peer, ready, a);
    run(conn_id, effects);
}

/// Called from JNI when VictorSuite reports a state change, output or exit for a session.
pub fn on_broker_event(sid: &str, ev: BrokerEvent) {
    let routed = REGISTRY.lock().unwrap().on_broker_event(sid, ev);
    if let Some((conn_id, resp)) = routed {
        respond(conn_id, resp);
    }
}

fn run(conn_id: i32, effects: Vec<Effect>) {
    for e in effects {
        let res = match &e {
            Effect::BrokerOpen { sid, peer, rows, cols } => Some((sid.clone(), scrap::android::call_main_service_victor_terminal("open", sid, peer, *cols as i32, *rows as i32, &[]))),
            Effect::BrokerInput { sid, data } => Some((sid.clone(), scrap::android::call_main_service_victor_terminal("input", sid, "", 0, 0, data))),
            Effect::BrokerResize { sid, rows, cols } => Some((sid.clone(), scrap::android::call_main_service_victor_terminal("resize", sid, "", *cols as i32, *rows as i32, &[]))),
            Effect::BrokerClose { sid, reason } => {
                let _ = scrap::android::call_main_service_victor_terminal("close", sid, reason, 0, 0, &[]);
                None
            }
            Effect::Respond(r) => {
                respond(conn_id, r.clone());
                None
            }
        };
        if let Some((sid, res)) = res {
            let code = match res {
                Ok(code) => code,
                Err(_) => "unavailable".to_owned(),
            };
            if !code.is_empty() {
                // Only an `open` failure ends the session; input/resize on a session VictorSuite already closed is just dropped.
                if matches!(e, Effect::BrokerOpen { .. }) {
                    on_broker_event(&sid, BrokerEvent::Rejected(code));
                }
            }
        }
    }
}

fn respond(conn_id: i32, r: Response) {
    let mut resp = TerminalResponse::new();
    match r {
        Response::Opened { terminal_id, success, message } => {
            let mut o = TerminalOpened::new();
            o.terminal_id = terminal_id;
            o.success = success;
            o.message = message;
            o.replay_terminal_output = false; // a phone terminal never replays buffered output
            resp.set_opened(o);
        }
        Response::Data { terminal_id, data } => {
            let mut d = TerminalData::new();
            d.terminal_id = terminal_id;
            d.data = data.into();
            resp.set_data(d);
        }
        Response::Closed { terminal_id, exit_code } => {
            let mut c = TerminalClosed::new();
            c.terminal_id = terminal_id;
            c.exit_code = exit_code;
            resp.set_closed(c);
        }
        Response::Error { terminal_id, message } => {
            let mut er = TerminalError::new();
            er.terminal_id = terminal_id;
            er.message = message;
            resp.set_error(er);
        }
    }
    let mut msg = Message::new();
    msg.set_terminal_response(resp);
    if let Some(inner) = CONNS.lock().unwrap().get_mut(&conn_id) {
        inner.send(Arc::new(msg));
    }
}
