# RustDeskV private server kit

Run on an always-on Linux host (VictorLinux, Ubuntu PC or a dedicated server). Requires Python 3, Docker Engine and Docker Compose v2, using a **local** Docker context. This is a deployment wrapper around upstream RustDesk Server OSS, not a new server or a Pro implementation. See [upstream OSS Docker documentation](https://rustdesk.com/docs/en/self-host/rustdesk-server-oss/docker/) and the [1.1.16 source release](https://github.com/rustdesk/rustdesk-server/tree/1.1.16).

## 1. Choose a private interface

Choose the actual IPv4 address of your server on your LAN or Headscale/Tailscale network. Only RFC1918 or CGNAT IPv4 addresses are accepted. The same address must be reachable from every client; use the VPN address for cross-network access. Check with `ip -br address`.

Do not port-forward these services. Restrict access with your host/network firewall and VPN ACLs to approved devices. Docker-published ports may bypass some host firewall rules (including UFW); validate the effective Docker firewall rules and test from an untrusted network before relying on isolation. A private address alone does not authenticate a peer. Bridge networking may reduce direct-connect success compared with host networking; the relay remains available. No web client ports or administrative API are exposed.

## 2. Pin the upstream server image

Review the upstream release and verify its provenance before deploying. The classic GHCR image name and version-tag publishing are defined in the upstream 1.1.16 build workflow. The commands resolve the pulled image to an immutable registry digest; a digest pins bytes but does not by itself prove publisher identity.

```sh
docker pull ghcr.io/rustdesk/rustdesk-server:1.1.16
docker image inspect --format '{{range .RepoDigests}}{{println .}}{{end}}' ghcr.io/rustdesk/rustdesk-server:1.1.16
```

Copy the full `ghcr.io/rustdesk/rustdesk-server@sha256:...` reference from the output. Check it against the trusted registry release before using it. Review current security updates before selecting a version; 1.1.16 is the source version reviewed for this kit.

## 3. Configure and start

From the repository root:

```sh
cd deploy/private-server
read -r -p "Server private IPv4: " BIND_IP
read -r -p "Verified image digest reference: " IMAGE
python3 serverctl.py configure --bind-ip "$BIND_IP" --image "$IMAGE"
python3 serverctl.py check
python3 serverctl.py up
python3 serverctl.py status
```

The wrapper does not install Docker, open firewalls, alter routers, use host networking or configure a cloud account. It publishes TCP 21115/21116/21117 and UDP 21116 on the selected interface only. Both servers use the same persistent data directory and upstream `-k _` key enforcement; the relay may take time to become ready while the key is generated. The public key is not a user password or a substitute for VPN ACLs or per-device authentication.

## 4. Configure clients

```sh
python3 serverctl.py client-config
```

Enter the returned ID server, relay server and public key under RustDeskV Settings > Network. Leave API server blank. If key generation is still in progress, repeat after checking service status. If file permissions prevent reading the public key, an administrator may read **only** `state/data/id_ed25519.pub` locally. Never copy or disclose the private key. Keep screen sharing, input and terminal grants separate; use private production APK signing keys for trusted apps.

## 5. Stop, back up and recover

```sh
python3 serverctl.py down
```

This removes the containers but not the data. With services stopped, securely back up the entire `state` directory (contains server private key and database). Restrict backup access, encrypt it and do not commit it. Reconfiguration requires a deliberate offline edit of `state/config.json`; preserve the data and validate with `check` before restarting. Do not delete state just to upgrade. If the private key is compromised, rotate it deliberately and reconfigure all clients with the new independently verified public key.

## Verification

Run `python3 -m unittest -v` in this directory. The CI workflow also validates the Compose model without pulling or deploying an image. Before real use, verify the selected interface, test connection and relay from two authorized clients, confirm incorrect server keys are rejected, verify access from unauthorized networks is blocked, and test backup/restore. No production deployment or physical-device test has been performed as part of this change.
