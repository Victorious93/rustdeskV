# RustDeskV Manual

Your desktop. Anywhere. Self-hosted.

This manual explains, in plain English, how to install and use RustDeskV. You do not need to be technical to follow it.

Contents

1. What RustDeskV is
2. Installing the app (Android APK)
3. Android permissions and why they are needed
4. Setting your own server (ID server, relay, key)
5. Using RustDeskV over Headscale or Tailscale (direct IP)
6. Connecting to another device
7. Letting someone connect to your device
8. Knowing when you are being accessed, and stopping a session
9. Transferring files
10. Troubleshooting
11. Privacy and legal
12. License and credits


## 1. What RustDeskV is

RustDeskV is a remote desktop app. It lets you see and control one of your devices (a PC, laptop, or phone) from another device, and move files between them.

RustDeskV is the Victorious fleet's version of RustDesk, an open-source remote desktop app. The big difference: RustDeskV only talks to a server that **you** run. It never uses RustDesk's public servers, and it does not check RustDesk's website for updates.

A few words you will see:

- **ID**: a number that identifies your device, a bit like a phone number. You give it to someone so they can connect to you.
- **ID server** (also called rendezvous server or hbbs): the server that helps two devices find each other.
- **Relay server** (hbbr): the server that passes the connection along when the two devices cannot reach each other directly.
- **Key**: a code that proves you are talking to your real server and not an imposter.


## 2. Installing the app (Android APK)

RustDeskV is not in the Play Store. You install it from an APK file.

1. Get a privately signed RustDeskV APK from your fleet administrator or the project's GitHub Releases page. For isolated development only, use a `TEST-ONLY` APK from the relevant GitHub Actions run.
   - Most modern phones need the file ending in `arm64-v8a.apk`.
2. Open the APK on your phone.
3. If Android says it blocks apps from unknown sources, tap **Settings** and allow installs from the app you used to open the file (for example, your browser or Files app). Then go back and tap **Install**.
4. Open **RustDeskV** from your app drawer.

Updating: APKs signed with the same signing key can normally update in place. The shared debug key is public: anyone holding it can sign an APK with the same identity. Do not grant test builds trusted terminal access or use them on production devices. Verify the production signing certificate independently before granting trust in VictorSuite. If Android reports a signing conflict, do not uninstall blindly: confirm the new APK's origin and certificate, export needed settings, then uninstall the test build if migrating to a private release key. Re-establish trust for the new certificate; uninstalling resets app settings.

RustDeskV has its own app ID (`com.victorious.rustdeskv`), so it can sit next to the regular RustDesk app on the same phone.


## 3. Android permissions and why they are needed

RustDeskV only asks for permissions when you turn on a feature that needs them. You can see and change them on the **Share screen** tab under **Permissions**.

- **Screen capture.** Needed so the other device can see your screen. Android shows its own warning ("RustDeskV will start capturing everything displayed on your screen"). You must tap **Start now** each time. Turning this on also starts the RustDeskV service so others can ask to connect.
- **Input control (Accessibility).** Needed so the other device can tap, swipe, and type on your phone. Android only allows this through the Accessibility settings: go to **Settings > Accessibility > Installed apps (or Downloaded apps) > RustDeskV Input** and turn it on. Android may warn that the app can "view and control your screen"; that is exactly what remote control means. Leave it off if you only want to show your screen.
- **File access (Transfer file).** Needed to send and receive files. RustDeskV asks Android for access to your storage.
- **Notifications.** Used to show that RustDeskV is running and when someone is connected.
- **Microphone (optional).** Only if you want to share sound.
- **Battery optimization exception (optional).** Keeps Android from shutting RustDeskV down in the background, so you stay reachable.

You can turn any of these off again at any time in Android's Settings.


## 4. Setting your own server (ID server, relay, key)

RustDeskV needs your own RustDesk server (hbbs and hbbr). If your copy of the app was built with a server already filled in, you can skip this. If not, the **Share screen** tab will say it is not ready until you set one.

You need three things from whoever runs the server:

- The server address (an IP like `100.64.0.10` or a name like `rd.example.com`).
- The relay address (often the same address).
- The key: the contents of the file `id_ed25519.pub` on the server. It is a line of letters and numbers ending in `=`.

To enter them:

1. Open RustDeskV and go to **Settings**.
2. Tap **ID/Relay Server**.
3. Fill in:
   - **ID server**: your server address, for example `100.64.0.10` (the app adds the default port 21116 if you leave it out).
   - **Relay server**: for example `100.64.0.10` (default port 21117).
   - **API server**: leave empty unless you were given one.
   - **Key**: paste the key.
4. Tap **OK**.

After a few seconds the **Share screen** tab should say **Ready**. Both devices that want to connect must use the same server and key.

On a PC, the same settings are under **Settings > Network > ID/Relay server**.


## 5. Using RustDeskV over Headscale or Tailscale (direct IP)

If your devices are on the same Headscale or Tailscale network (or the same home network), they can connect straight to each other by IP address, with no ID server needed for that connection.

On the device you want to reach:

1. Make sure it is connected to your Tailscale/Headscale network and note its Tailscale IP (it usually starts with `100.`).
2. In RustDeskV, go to **Settings** and turn on **Direct IP access**. The default port is **21118**.

On the device you are connecting from:

1. Make sure it is also on the same Tailscale/Headscale network.
2. In the connect box, type the other device's IP address instead of its ID, for example `100.64.0.23`. If you changed the port, add it: `100.64.0.23:21118`.
3. Connect as usual.

Tip: you can also run your ID and relay server on a machine inside your Tailscale network and use its `100.x` address in step 4 above. Then everything stays inside your private network.


## 6. Connecting to another device

1. Ask the person at the other device (or yourself, if it is your own device) for its **ID** and **one-time password**. They are shown on the other device's main screen.
2. On your device, type the ID in the **Remote ID** box and tap the connect arrow.
3. Enter the password when asked.
4. If the other side has to approve connections, wait for them to tap **Accept**.

Once connected you can see the screen. On a phone, use the toolbar to switch between touch mode and mouse mode, open the keyboard, or end the session.


## 7. Letting someone connect to your device

On Android:

1. Open the **Share screen** tab.
2. Tap **Start service** and allow **Screen capture** when Android asks.
3. Give the other person your **ID** and the **one-time password** shown on screen. Only share these with people you trust.
4. When they try to connect, RustDeskV shows a request with their name and ID. Tap **Accept** to let them in, or **Dismiss** to refuse.

If you also turned on **Input control**, they can control your phone. If not, they can only watch.


## 8. Knowing when you are being accessed, and stopping a session

While someone is connected:

- A RustDeskV notification stays in your notification bar.
- The **Share screen** tab lists each connected person, with their name and ID.
- Android also shows its own screen-sharing indicator (for example a colored icon or chip in the status bar, depending on your phone).

To stop a session:

- Tap **Disconnect** next to the person on the **Share screen** tab, or
- Tap **Stop service** to end every session and stop anyone new from connecting, or
- Turn off **Screen capture** in the Permissions list.

If you ever see a connection you did not expect, tap **Stop service** right away, then change your password in RustDeskV's settings.


## 9. Transferring files

1. Connect to the other device as in section 6, but choose **Transfer file** instead of a normal connection.
2. You will see your files on one side and the other device's files on the other.
3. Select files and tap the send arrow to copy them across.

On Android, the device that receives files must have granted **file access** (see section 3).


## 10. Troubleshooting

**"Not ready" on the Share screen tab.** RustDeskV cannot reach your ID server. Check the server address and key in **Settings > ID/Relay Server**, check your internet or Tailscale connection, and make sure the server is running. If no server was built into your copy of the app, you must enter one (section 4).

**The other side cannot find my ID.** Both devices must use the same ID server and key.

**Connected, but I can only watch, not control.** Turn on **Input control** (Accessibility > RustDeskV Input) on the Android device being controlled.

**Black screen.** Screen capture was refused or stopped. Tap **Start service** again and allow screen capture. Some apps (banking, DRM video) block screen capture on purpose.

**RustDeskV stops when the screen turns off.** Allow the battery optimization exception and, on some phones, lock RustDeskV in the recent apps list.

**"App not installed" when updating.** The new APK has a different signature. Uninstall the old one first.

**Direct IP connection fails.** Make sure **Direct IP access** is on, both devices are on the same Tailscale/Headscale network, and you used the right `100.x` address and port (21118 by default).


## 11. Privacy and legal

- RustDeskV only connects to the server you set (or the one built into your copy). It never falls back to RustDesk's public servers and does not check rustdesk.com for updates.
- Your screen, input, and files travel between your devices through your own server (or directly between them), and the connection is encrypted. Setting your server key (section 4) is what lets RustDeskV check it is talking to your real server.
- **Only use RustDeskV on devices you own, or that you have clear permission to access.** Accessing someone else's device without permission is illegal in most countries and can be a crime. Always tell people when you are connecting to a device they use.
- Never share your ID and password with someone who contacts you out of the blue, for example a caller claiming to be "tech support". Scammers use remote desktop apps this way.


## 12. License and credits

RustDeskV is forked from RustDesk (https://github.com/rustdesk/rustdesk), created by Purslane Tech Pte. Ltd. and the RustDesk contributors. Like RustDesk, it is free software licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). You can get the full source code at https://github.com/Victorious93/rustdeskV.

RustDeskV is not affiliated with or endorsed by RustDesk.
