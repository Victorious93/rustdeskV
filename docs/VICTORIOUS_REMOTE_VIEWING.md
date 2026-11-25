# Victorious: phone→PC, phone→phone, and sessions across two networks

Work by Victorious. This is a review of what the fork already does, with code references, and how to use it safely. **No code was changed for it, and nothing here has been built or run on a device.** A Rust/Flutter build was not feasible on the review box.

## What already exists (inspected)

| Need | Where it lives | Status |
|---|---|---|
| Phone views/controls a PC | Flutter mobile client `flutter/lib/mobile/pages/remote_page.dart`, `connection_page.dart` | upstream, unchanged |
| Phone shares its screen (phone→phone, phone→PC) | `flutter/lib/mobile/pages/server_page.dart`; Android `MainService.kt` (MediaProjection, l.355/451) | upstream |
| **Screen-capture consent** | `PermissionRequestTransparentActivity.kt` asks Android's MediaProjection dialog; capture cannot start without it, and Android shows the cast indicator | upstream |
| **Input is a separate grant** | `InputService.kt` (an `AccessibilityService`, l.73) must be enabled by the owner in Android Settings. Without it a peer can only view | upstream |
| Per-connection keyboard/mouse permission | host side `src/server/connection.rs` `ipc::Data::SwitchPermission` (l.803, l.900); default `OPTION_ENABLE_KEYBOARD` (`libs/base/src/config/keys.rs` l.38) | upstream |
| **View-only** from the viewer | `src/client.rs` `toggle_option("view-only")` (l.3290–3306) | upstream |
| Approval mode / password | `OPTION_APPROVE_MODE`, `OPTION_VERIFICATION_METHOD` (`keys.rs` l.73–74) | upstream |
| **Stop/revoke from either end** | viewer: close the session; host phone: Disconnect in the connection card or stop sharing (`MainService.stopCapture`, l.631); host PC: connection manager Close (`connection.rs` `ipc::Data::Close`, l.766) | upstream |
| Two networks | rendezvous + hole-punch, falling back to relay (`src/rendezvous_mediator.rs`); `OPTION_DIRECT_SERVER` (`keys.rs` l.293) for LAN/VPN direct | upstream |
| VictorSuite terminal hand-off | `VictorTerminalBridge.kt`, `VictorTerminalTransportService.kt` (PR #2) | fork |

## How to use it

1. **Phone → PC.** Install the RustDesk client on the PC and run it. On the phone, enter the PC's ID and accept on the PC, or use its one-time password. Tap the eye in the session toolbar for view-only.
2. **Phone → phone.** On the phone that shares: open Share screen, tap Start, and accept Android's "start recording/casting" dialog. Enable *Input control* (the accessibility service) only if the other phone should control it. The phone that views connects by ID. Each incoming connection appears as a card where you can turn keyboard/mouse off or Disconnect.
3. **Across two networks.**
   - *Direct:* both ends on the same LAN or VPN (WireGuard/Tailscale). Enable direct IP access on the host. Never port-forward it to the internet.
   - *Relay:* run your own `hbbs`/`hbbr` (rustdesk-server) and set its ID/relay server and key on both ends. The traffic stays end-to-end encrypted. The relay only forwards it.

## Safety rules

- Capture always needs the phone owner's MediaProjection consent per session. Do not add auto-accept.
- Control (input) stays a separate Android accessibility grant, plus the per-connection keyboard switch. Prefer view-only by default.
- Keep `adb`, the Victor fleet coordinator admin API and any admin UI **off the public internet**. Bind them to localhost/LAN/VPN only. Use RustDesk's relay for cross-network sessions, not port forwards.
- Revocation: disconnect from either end. On the phone, stopping sharing ends all sessions. Removing the accessibility grant removes control immediately.

## Shortest remaining checks

- Build the Android APK (`flutter build apk`) and the desktop client on a machine with Rust + Flutter.
- On two real phones: share → consent dialog appears; view without the accessibility grant (input must fail); enable the grant; flip the per-connection keyboard switch; disconnect from each end.
- Cross-network: a phone on mobile data to a PC on Wi-Fi via a self-hosted relay, and then via a VPN with direct access.
