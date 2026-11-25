# Terminals through VictorSuite (rustdeskV)

Every terminal that touches a Victorious phone is VictorSuite's: on a phone **client** rustdeskV hands the terminal to VictorSuite's
view, and on a phone **host** the shell is VictorSuite's PTY behind its shell broker. rustdeskV never starts a shell on a phone.

## Phone as client (Android -> PC) — already merged (#2)
`Terminal.victorsuite_handoff = "victorsuite/1"`; the host refuses Android/iOS clients that do not declare it (`terminal_policy.rs`).
The Android app exposes `VictorTerminalTransportService` (AIDL `IRemoteTerminalTransport`) to VictorSuite's *Remote shell* view.

## Phone as host (PC or another phone -> this phone) — this change
```
rustdesk peer --TerminalAction--> connection.rs (authorised Terminal login only)
   -> android_terminal_glue.rs -> android_terminal_host.rs (pure rules)
   -> JNI scrap::android::call_main_service_victor_terminal -> MainService.rustVictorTerminal
   -> VictorHostTerminal.kt -> com.termux ShellBrokerService (IShellBroker API 1)
   -> owner approves on the phone (binding = peer + target + session + privilege) -> VictorSuite PTY
VictorSuite IShellSessionCallback -> FFI.onVictorTerminalEvent -> glue -> TerminalResponse to that same connection only
```
Rules (unit-tested in `src/server/android_terminal_host.rs`):
* Terminal is advertised and admitted only when VictorSuite is installed, its certificate matches the pin the owner set in
  rustdeskV, broker API >= 1, and *Remote shell access* is on in VictorSuite (checked at login **and** at every open).
* Privilege is always `user`. Root needs VictorSuite's separate privileged switch and is never requested by rustdeskV.
* Each open gets a fresh broker session id; a phone terminal is never persistent, so a reconnect never resumes, replays output
  or changes target, and VictorSuite refuses a reused id.
* Input before the owner approves, or after close/revoke, is dropped, never queued. Ctrl-C is the byte 0x03 to the PTY.
* Ending the rustdesk connection closes every VictorSuite session it opened; Revoke/Disconnect in VictorSuite closes the
  rustdesk terminal.
* Terminal bytes are passed through only; nothing logs or stores them.
* Screen share and file transfer are unchanged; `com.termux.RUN_COMMAND` is not used by this path (no downgrade).

The peer id shown to the owner is the rustdesk id of the authorised connection (password/2FA-authenticated session; the id itself
is the one the peer reports).

## Owner setup
1. rustdeskV: trust VictorSuite (pin its certificate; same pin as the client hand-off).
2. VictorSuite → *Remote shell access*: switch on, trust rustdeskV with **Terminal** scope after checking its fingerprint.
3. rustdeskV permissions: *Enable terminal* on.
4. From the PC: open Terminal to the phone's rustdesk id; approve on the phone. Stop: close the tab, Disconnect in rustdeskV, or
   Revoke in VictorSuite.

## Status
| Part | Implemented | Built | Tested |
|---|---|---|---|
| `android_terminal_host.rs` rules | yes | `rustc --test` (host) | 10 unit tests pass |
| `terminal_policy.rs` | yes (#2) | `rustc --test` | 5 pass |
| JNI glue (`android_terminal_glue.rs`, scrap ffi, flutter_ffi), connection wiring | yes | **not compiled for Android** (needs vcpkg deps + flutter_rust_bridge codegen) | no |
| `VictorHostTerminal.kt`, AIDL copy, MainService hooks | yes | **not compiled** (needs the Flutter Android build) | no |
| End-to-end on a phone or emulator | — | — | **not run** |

Shortest remaining check: build the Android APK in the Full Flutter CI (currently `startup_failure`) or a local Flutter+vcpkg
setup, install with VictorSuite (PR #114) on two emulators, trust each other, open Terminal from a desktop rustdesk to the phone id.
