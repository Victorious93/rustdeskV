//! Terminal admission policy: every terminal opened on a Victorious phone UI goes through VictorSuite.
//!
//! The *host* enforces this so an older or modified client cannot bypass it: a Terminal login from an Android/iOS client must
//! declare the `victorsuite/1` hand-off (its terminal is rendered by VictorSuite, which owns the shell UI on the phone). Desktop clients
//! are unaffected. Pure functions only, so the rules are unit-tested without the rest of the crate
//! (`rustc --test src/terminal_policy.rs && ./terminal_policy`).

/// Value of `Terminal.victorsuite_handoff` that a hand-off capable client sends.
pub const HANDOFF_V1: &str = "victorsuite/1";

#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    Allow,
    Deny(&'static str),
}

fn is_phone(platform: &str) -> bool {
    platform.eq_ignore_ascii_case("android") || platform.eq_ignore_ascii_case("ios")
}

/// `client_platform` is `LoginRequest.my_platform`; `handoff` is `Terminal.victorsuite_handoff`.
/// `host_is_phone` / `victorsuite_ready` describe THIS host: a phone host can only serve a terminal through VictorSuite's shell broker.
pub fn admit_terminal(client_platform: &str, handoff: &str, host_is_phone: bool, victorsuite_ready: bool) -> Admission {
    if is_phone(client_platform) && handoff != HANDOFF_V1 {
        return Admission::Deny("Terminal from a phone must open in VictorSuite. Update rustdeskV and VictorSuite on the connecting phone.");
    }
    if host_is_phone && !victorsuite_ready {
        return Admission::Deny("VictorSuite's shell broker is not ready on this phone (missing, untrusted, outdated or switched off).");
    }
    Admission::Allow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_android_client_without_handoff_is_refused() {
        assert!(matches!(admit_terminal("Android", "", false, false), Admission::Deny(_)));
        assert!(matches!(admit_terminal("android", "victorsuite/0", false, false), Admission::Deny(_)));
        assert!(matches!(admit_terminal("iOS", "", false, false), Admission::Deny(_)));
    }

    #[test]
    fn handoff_capable_android_client_is_admitted_on_a_desktop_host() {
        assert_eq!(admit_terminal("Android", HANDOFF_V1, false, false), Admission::Allow);
    }

    #[test]
    fn desktop_clients_are_not_affected() {
        for p in ["Windows", "Linux", "Mac OS", ""] {
            assert_eq!(admit_terminal(p, "", false, false), Admission::Allow, "{p}");
        }
    }

    #[test]
    fn a_phone_host_needs_a_ready_victorsuite_broker_even_for_desktop_clients() {
        assert!(matches!(admit_terminal("Linux", "", true, false), Admission::Deny(_)));
        assert_eq!(admit_terminal("Linux", "", true, true), Admission::Allow);
    }

    #[test]
    fn handoff_value_must_match_exactly() {
        assert!(matches!(admit_terminal("Android", "VictorSuite/1", false, false), Admission::Deny(_)));
        assert!(matches!(admit_terminal("Android", " victorsuite/1", false, false), Admission::Deny(_)));
    }
}
