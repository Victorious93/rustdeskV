//! RustDeskV fleet defaults.
//!
//! Upstream RustDesk falls back to its public servers (`rs-ny.rustdesk.com` and
//! its public key) when no server is configured. Those fallbacks live in the
//! `hbb_common` submodule, which is shared with upstream and not owned by this
//! fork, so instead of patching it we override the defaults here, in the main
//! crate, right after the custom client config is loaded.
//!
//! The values come from compile-time environment variables. When a variable is
//! not set at build time the fallback is empty, never rustdesk.com:
//!
//! * `RUSTDESKV_RENDEZVOUS_SERVER` - ID / rendezvous server (hbbs), `host[:port]`
//! * `RUSTDESKV_RELAY_SERVER`      - relay server (hbbr), `host[:port]`
//! * `RUSTDESKV_RS_PUB_KEY`        - public key of the server (`id_ed25519.pub`)
//! * `RUSTDESKV_API_SERVER`        - optional API server URL
//!
//! The values are written to `DEFAULT_SETTINGS`, so anything the user sets in
//! Settings > Network (or a signed `custom.txt`) still wins.

use base::config::keys;
use hbb_common::{config, log};

/// Placeholder rendezvous host used when no server was compiled in and the
/// user has not set one yet. `.invalid` is reserved by RFC 6761 and never
/// resolves, so an unconfigured build stays offline (it shows as not ready)
/// instead of silently falling back to RustDesk's public server.
pub const UNCONFIGURED_RENDEZVOUS_SERVER: &str = "rustdeskv-unconfigured.invalid";

const fn env_or_empty(v: Option<&'static str>) -> &'static str {
    match v {
        Some(v) => v,
        None => "",
    }
}

pub const RENDEZVOUS_SERVER: &str = env_or_empty(option_env!("RUSTDESKV_RENDEZVOUS_SERVER"));
pub const RELAY_SERVER: &str = env_or_empty(option_env!("RUSTDESKV_RELAY_SERVER"));
pub const RS_PUB_KEY: &str = env_or_empty(option_env!("RUSTDESKV_RS_PUB_KEY"));
pub const API_SERVER: &str = env_or_empty(option_env!("RUSTDESKV_API_SERVER"));

/// Returns true when a rendezvous server was compiled into this build.
pub fn has_builtin_server() -> bool {
    !RENDEZVOUS_SERVER.trim().is_empty()
}

/// Apply the RustDeskV server defaults. Safe to call more than once.
pub fn apply_defaults() {
    {
        let mut defaults = match config::DEFAULT_SETTINGS.write() {
            Ok(d) => d,
            Err(e) => {
                log::error!("RustDeskV: failed to lock default settings: {e}");
                return;
            }
        };
        for (key, value) in [
            (keys::OPTION_CUSTOM_RENDEZVOUS_SERVER, RENDEZVOUS_SERVER),
            (keys::OPTION_RELAY_SERVER, RELAY_SERVER),
            (keys::OPTION_KEY, RS_PUB_KEY),
            (keys::OPTION_API_SERVER, API_SERVER),
        ] {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            // A signed custom.txt loaded before us takes precedence.
            defaults
                .entry(key.to_owned())
                .or_insert_with(|| value.to_owned());
        }
    }
    if !has_builtin_server() {
        if let Ok(mut prod) = config::PROD_RENDEZVOUS_SERVER.write() {
            if prod.is_empty() {
                *prod = UNCONFIGURED_RENDEZVOUS_SERVER.to_owned();
            }
        }
        log::info!("RustDeskV: no built-in server; set your ID/relay server in Settings > Network");
    }
}
