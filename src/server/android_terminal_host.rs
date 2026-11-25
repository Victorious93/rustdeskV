//! Android HOST terminal: a rustdesk Terminal connection to a phone is served by VictorSuite's shell broker, never by a shell that
//! rustdeskV starts itself. This file is the pure session core (no JNI, no protobuf) so its rules are unit-tested on any host
//! (`rustc --edition 2021 --test src/server/android_terminal_host.rs && ./android_terminal_host`). The Android glue lives in
//! `connection.rs` (cfg android) and `flutter_ffi.rs` (`Java_ffi_FFI_onVictorTerminalEvent`); the broker side is Kotlin
//! (`VictorHostTerminal.kt`, binding `com.termux.victor.shellbroker.IShellBroker`).
//!
//! Rules enforced here:
//! * one broker session per (connection, terminal id); the broker session id is fresh for every open, so a reconnect never
//!   resumes or replays anything and the broker's own duplicate check refuses a reused id;
//! * input is forwarded only while the broker reports the session `open`; earlier or later bytes are dropped, never queued;
//! * the owner approves on the phone (the broker's dialog, bound to peer + target + session + privilege); privilege is always
//!   `user` from rustdesk (a privileged shell needs VictorSuite's separate switch and is not requested here);
//! * a connection that ends closes all of its broker sessions; a broker `revoked`/`closed`/`expired` ends the rustdesk terminal;
//! * terminal contents are passed through and never stored or logged by this module.

use std::collections::HashMap;

/// Most terminals one rustdesk connection may hold on a phone (the broker has its own global cap too).
pub const MAX_TERMINALS_PER_CONN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Open { terminal_id: i32, rows: u32, cols: u32 },
    Data { terminal_id: i32, data: Vec<u8> },
    Resize { terminal_id: i32, rows: u32, cols: u32 },
    Close { terminal_id: i32 },
}

/// What the Android glue must do. `Broker*` go to VictorSuite through JNI; `Respond` goes back to the rustdesk peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    BrokerOpen { sid: String, peer: String, rows: u32, cols: u32 },
    BrokerInput { sid: String, data: Vec<u8> },
    BrokerResize { sid: String, rows: u32, cols: u32 },
    BrokerClose { sid: String, reason: &'static str },
    Respond(Response),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Opened { terminal_id: i32, success: bool, message: String },
    Data { terminal_id: i32, data: Vec<u8> },
    Closed { terminal_id: i32, exit_code: i32 },
    Error { terminal_id: i32, message: String },
}

/// Broker events, as delivered by `IShellSessionCallback`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerEvent {
    /// `awaiting_approval | open | denied | closed | failed | revoked | expired`
    State { state: String, reason: String },
    Output(Vec<u8>),
    Exit(i32),
    /// `IShellBroker.open` returned a reject code synchronously.
    Rejected(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitingApproval,
    Open,
}

#[derive(Debug)]
struct Term {
    conn_id: i32,
    terminal_id: i32,
    phase: Phase,
}

/// All Android host terminals of this process. Owned by the server behind a mutex.
#[derive(Debug, Default)]
pub struct Registry {
    by_sid: HashMap<String, Term>,
    next: u64,
    salt: u64,
}

impl Registry {
    /// `salt` must be unpredictable (e.g. random per process start) so session ids cannot be guessed across restarts.
    pub fn new(salt: u64) -> Self {
        Self { by_sid: HashMap::new(), next: 0, salt }
    }

    fn sid_of(&self, conn_id: i32, terminal_id: i32) -> Option<String> {
        self.by_sid.iter().find(|(_, t)| t.conn_id == conn_id && t.terminal_id == terminal_id).map(|(s, _)| s.clone())
    }

    pub fn live_count(&self, conn_id: i32) -> usize {
        self.by_sid.values().filter(|t| t.conn_id == conn_id).count()
    }

    /// `peer` is the authenticated rustdesk peer id of this connection (shown to the owner and bound into the approval).
    pub fn on_action(&mut self, conn_id: i32, peer: &str, broker_ready: bool, action: Action) -> Vec<Effect> {
        match action {
            Action::Open { terminal_id, rows, cols } => {
                if !broker_ready {
                    return vec![Effect::Respond(Response::Opened {
                        terminal_id,
                        success: false,
                        message: "VictorSuite's shell broker is not ready on this phone (missing, untrusted, outdated or switched off).".into(),
                    })];
                }
                if self.sid_of(conn_id, terminal_id).is_some() {
                    return vec![Effect::Respond(Response::Error { terminal_id, message: "terminal already open".into() })];
                }
                if self.live_count(conn_id) >= MAX_TERMINALS_PER_CONN {
                    return vec![Effect::Respond(Response::Opened { terminal_id, success: false, message: "too many terminals on this connection".into() })];
                }
                if peer.trim().is_empty() {
                    return vec![Effect::Respond(Response::Opened { terminal_id, success: false, message: "unauthenticated peer".into() })];
                }
                self.next += 1;
                let sid = format!("rdv-{:x}-{:x}-{}-{}", self.salt, self.next, conn_id, terminal_id);
                self.by_sid.insert(sid.clone(), Term { conn_id, terminal_id, phase: Phase::AwaitingApproval });
                let (rows, cols) = (rows.clamp(1, 1000), cols.clamp(1, 1000));
                vec![Effect::BrokerOpen { sid, peer: format!("rustdesk:{}", peer.trim()), rows, cols }]
            }
            Action::Data { terminal_id, data } => match self.sid_of(conn_id, terminal_id) {
                // Forwarded now or dropped: never queued, so nothing typed before approval or after close can run later.
                Some(sid) if self.by_sid[&sid].phase == Phase::Open && !data.is_empty() => vec![Effect::BrokerInput { sid, data }],
                _ => vec![],
            },
            Action::Resize { terminal_id, rows, cols } => match self.sid_of(conn_id, terminal_id) {
                Some(sid) => vec![Effect::BrokerResize { sid, rows: rows.clamp(1, 1000), cols: cols.clamp(1, 1000) }],
                None => vec![],
            },
            Action::Close { terminal_id } => match self.sid_of(conn_id, terminal_id) {
                Some(sid) => {
                    self.by_sid.remove(&sid);
                    vec![Effect::BrokerClose { sid, reason: "closed by the rustdesk peer" }, Effect::Respond(Response::Closed { terminal_id, exit_code: 0 })]
                }
                None => vec![],
            },
        }
    }

    /// The rustdesk connection ended (disconnect, logout, permission revoked by the owner in rustdeskV).
    pub fn on_disconnect(&mut self, conn_id: i32) -> Vec<Effect> {
        let sids: Vec<String> = self.by_sid.iter().filter(|(_, t)| t.conn_id == conn_id).map(|(s, _)| s.clone()).collect();
        sids.into_iter()
            .map(|sid| {
                self.by_sid.remove(&sid);
                Effect::BrokerClose { sid, reason: "rustdesk connection ended" }
            })
            .collect()
    }

    /// Returns the connection to answer and what to send it. Events for unknown sessions are ignored.
    pub fn on_broker_event(&mut self, sid: &str, ev: BrokerEvent) -> Option<(i32, Response)> {
        let t = self.by_sid.get_mut(sid)?;
        let (conn, tid) = (t.conn_id, t.terminal_id);
        match ev {
            BrokerEvent::State { state, reason } => match state.as_str() {
                "awaiting_approval" => None,
                "open" => {
                    if t.phase == Phase::Open {
                        return None;
                    }
                    t.phase = Phase::Open;
                    Some((conn, Response::Opened { terminal_id: tid, success: true, message: String::new() }))
                }
                "denied" | "failed" | "expired" if t.phase == Phase::AwaitingApproval => {
                    self.by_sid.remove(sid);
                    Some((conn, Response::Opened { terminal_id: tid, success: false, message: reason_or(&reason, &state) }))
                }
                "closed" | "revoked" | "denied" | "failed" | "expired" => {
                    self.by_sid.remove(sid);
                    Some((conn, Response::Closed { terminal_id: tid, exit_code: if state == "closed" { 0 } else { -1 } }))
                }
                _ => None,
            },
            BrokerEvent::Output(data) if t.phase == Phase::Open => Some((conn, Response::Data { terminal_id: tid, data })),
            BrokerEvent::Output(_) => None,
            BrokerEvent::Exit(code) => {
                self.by_sid.remove(sid);
                Some((conn, Response::Closed { terminal_id: tid, exit_code: code }))
            }
            BrokerEvent::Rejected(code) => {
                self.by_sid.remove(sid);
                Some((conn, Response::Opened { terminal_id: tid, success: false, message: format!("VictorSuite refused the terminal: {code}") }))
            }
        }
    }
}

fn reason_or(reason: &str, state: &str) -> String {
    if reason.trim().is_empty() {
        format!("VictorSuite: {state}")
    } else {
        format!("VictorSuite: {state}: {reason}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(r: &mut Registry, conn: i32, tid: i32) -> String {
        match r.on_action(conn, "123456789", true, Action::Open { terminal_id: tid, rows: 24, cols: 80 }).as_slice() {
            [Effect::BrokerOpen { sid, peer, rows: 24, cols: 80 }] => {
                assert_eq!(peer, "rustdesk:123456789");
                sid.clone()
            }
            other => panic!("{other:?}"),
        }
    }

    fn st(s: &str) -> BrokerEvent {
        BrokerEvent::State { state: s.into(), reason: String::new() }
    }

    #[test]
    fn not_ready_broker_fails_closed_and_starts_nothing() {
        let mut r = Registry::new(7);
        let e = r.on_action(1, "p", false, Action::Open { terminal_id: 0, rows: 24, cols: 80 });
        assert!(matches!(e.as_slice(), [Effect::Respond(Response::Opened { success: false, .. })]));
        assert_eq!(r.live_count(1), 0);
    }

    #[test]
    fn input_before_approval_is_dropped_not_queued() {
        let mut r = Registry::new(7);
        let sid = open(&mut r, 1, 0);
        assert!(r.on_action(1, "p", true, Action::Data { terminal_id: 0, data: b"rm -rf x\n".to_vec() }).is_empty());
        assert_eq!(r.on_broker_event(&sid, st("awaiting_approval")), None);
        assert_eq!(r.on_broker_event(&sid, st("open")), Some((1, Response::Opened { terminal_id: 0, success: true, message: String::new() })));
        // The earlier keystrokes are gone; only new input flows.
        assert_eq!(r.on_action(1, "p", true, Action::Data { terminal_id: 0, data: b"ls\n".to_vec() }), vec![Effect::BrokerInput { sid, data: b"ls\n".to_vec() }]);
    }

    #[test]
    fn ctrl_c_resize_output_and_exit_flow_through_an_open_session() {
        let mut r = Registry::new(7);
        let sid = open(&mut r, 1, 3);
        r.on_broker_event(&sid, st("open"));
        assert_eq!(r.on_action(1, "p", true, Action::Data { terminal_id: 3, data: vec![0x03] }), vec![Effect::BrokerInput { sid: sid.clone(), data: vec![0x03] }]);
        assert_eq!(r.on_action(1, "p", true, Action::Resize { terminal_id: 3, rows: 50, cols: 5000 }), vec![Effect::BrokerResize { sid: sid.clone(), rows: 50, cols: 1000 }]);
        assert_eq!(r.on_broker_event(&sid, BrokerEvent::Output(b"$ ".to_vec())), Some((1, Response::Data { terminal_id: 3, data: b"$ ".to_vec() })));
        assert_eq!(r.on_broker_event(&sid, BrokerEvent::Exit(130)), Some((1, Response::Closed { terminal_id: 3, exit_code: 130 })));
        assert_eq!(r.live_count(1), 0);
        assert!(r.on_action(1, "p", true, Action::Data { terminal_id: 3, data: b"x".to_vec() }).is_empty());
    }

    #[test]
    fn owner_denial_is_reported_as_a_failed_open() {
        let mut r = Registry::new(7);
        let sid = open(&mut r, 1, 0);
        let got = r.on_broker_event(&sid, BrokerEvent::State { state: "denied".into(), reason: "owner tapped Deny".into() });
        assert!(matches!(got, Some((1, Response::Opened { success: false, ref message, .. })) if message.contains("Deny")));
        assert_eq!(r.live_count(1), 0);
    }

    #[test]
    fn revocation_on_the_phone_ends_the_rustdesk_terminal() {
        let mut r = Registry::new(7);
        let sid = open(&mut r, 1, 0);
        r.on_broker_event(&sid, st("open"));
        assert_eq!(r.on_broker_event(&sid, st("revoked")), Some((1, Response::Closed { terminal_id: 0, exit_code: -1 })));
        assert_eq!(r.on_broker_event(&sid, BrokerEvent::Output(b"late".to_vec())), None);
    }

    #[test]
    fn disconnect_closes_every_broker_session_of_that_connection_only() {
        let mut r = Registry::new(7);
        let a = open(&mut r, 1, 0);
        let b = open(&mut r, 1, 1);
        let other = open(&mut r, 2, 0);
        let mut closed: Vec<String> = r.on_disconnect(1).into_iter().map(|e| match e { Effect::BrokerClose { sid, .. } => sid, x => panic!("{x:?}") }).collect();
        closed.sort();
        let mut want = vec![a, b];
        want.sort();
        assert_eq!(closed, want);
        assert_eq!(r.live_count(2), 1);
        assert!(r.on_broker_event(&other, st("open")).is_some());
    }

    #[test]
    fn reconnect_gets_a_fresh_session_id_and_never_resumes_the_old_one() {
        let mut r = Registry::new(7);
        let first = open(&mut r, 1, 0);
        r.on_disconnect(1);
        let second = open(&mut r, 5, 0);
        assert_ne!(first, second);
        assert_eq!(r.on_broker_event(&first, BrokerEvent::Output(b"old".to_vec())), None);
    }

    #[test]
    fn duplicate_open_and_per_connection_cap_are_refused() {
        let mut r = Registry::new(7);
        open(&mut r, 1, 0);
        assert!(matches!(r.on_action(1, "p", true, Action::Open { terminal_id: 0, rows: 1, cols: 1 }).as_slice(), [Effect::Respond(Response::Error { .. })]));
        for t in 1..MAX_TERMINALS_PER_CONN as i32 {
            open(&mut r, 1, t);
        }
        assert!(matches!(r.on_action(1, "p", true, Action::Open { terminal_id: 99, rows: 1, cols: 1 }).as_slice(), [Effect::Respond(Response::Opened { success: false, .. })]));
    }

    #[test]
    fn a_synchronous_broker_reject_is_reported_and_forgotten() {
        let mut r = Registry::new(7);
        let sid = open(&mut r, 1, 0);
        assert!(matches!(r.on_broker_event(&sid, BrokerEvent::Rejected("scope_not_granted".into())), Some((1, Response::Opened { success: false, .. }))));
        assert_eq!(r.live_count(1), 0);
    }

    #[test]
    fn empty_peer_is_refused() {
        let mut r = Registry::new(7);
        assert!(matches!(r.on_action(1, " ", true, Action::Open { terminal_id: 0, rows: 1, cols: 1 }).as_slice(), [Effect::Respond(Response::Opened { success: false, .. })]));
    }
}
