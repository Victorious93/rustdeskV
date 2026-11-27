#!/usr/bin/env python3
"""Manage a private, locally hosted RustDesk OSS server. No automatic firewall changes."""
import argparse
import base64
import binascii
import ipaddress
import json
import os
from pathlib import Path
import re
import subprocess
import sys

HERE = Path(__file__).resolve().parent
STATE = HERE / "state"
CONFIG = STATE / "config.json"
IMAGE_RE = re.compile(r"ghcr\.io/rustdesk/rustdesk-server@sha256:[0-9a-f]{64}")
NETWORKS = tuple(ipaddress.ip_network(n) for n in (
    "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "100.64.0.0/10"
))


def validate(ip, image):
    addr = ipaddress.IPv4Address(ip)
    if not any(addr in network for network in NETWORKS):
        raise ValueError("bind address must be an RFC1918 or CGNAT/private VPN IPv4 address")
    if not IMAGE_RE.fullmatch(image):
        raise ValueError("use the official classic GHCR image pinned by SHA-256 digest")
    return {"bind_ip": str(addr), "image": image}


def configure(ip, image):
    config = validate(ip, image)
    STATE.mkdir(mode=0o700, exist_ok=True)
    STATE.chmod(0o700)
    (STATE / "data").mkdir(mode=0o700, exist_ok=True)
    # Refuse to replace configuration or reset a running server's identity.
    fd = os.open(CONFIG, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(config, stream, indent=2)
        stream.write("\n")


def load_config():
    with CONFIG.open() as stream:
        c = json.load(stream)
    return validate(c["bind_ip"], c["image"])


def compose_args(config, args):
    env = os.environ.copy()
    # Do not let inherited interpolation variables override validated config.
    env.update(RUSTDESKV_SERVER_IMAGE=config["image"], RUSTDESKV_BIND_IP=config["bind_ip"])
    command = ["docker", "compose", "--project-name", "rustdeskv-private",
               "--project-directory", str(HERE), "-f", str(HERE / "compose.yml"), *args]
    return command, env


def compose(config, *args):
    command, env = compose_args(config, args)
    subprocess.run(command, env=env, cwd=HERE, check=True)


def public_key(path):
    key = path.read_text(encoding="ascii").strip()
    if len(base64.b64decode(key, validate=True)) != 32:
        raise ValueError("server public key is not a 32-byte Ed25519 key")
    return key


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    setup = commands.add_parser("configure", help="create configuration once")
    setup.add_argument("--bind-ip", required=True)
    setup.add_argument("--image", required=True, help="official GHCR image with verified digest")
    for name in ("check", "up", "status", "down", "client-config"):
        commands.add_parser(name)
    args = parser.parse_args()
    try:
        if args.command == "configure":
            configure(args.bind_ip, args.image)
            print("Configuration saved. Verify firewall/VPN ACLs, then run check and up.")
            return 0
        c = load_config()
        if args.command == "check":
            compose(c, "config", "--quiet")
        elif args.command == "up":
            compose(c, "config", "--quiet")
            compose(c, "up", "-d")
        elif args.command == "status":
            compose(c, "ps")
        elif args.command == "down":
            compose(c, "down")  # preserves data and keys
        else:
            key = public_key(STATE / "data" / "id_ed25519.pub")
            print(json.dumps({"id_server": c["bind_ip"] + ":21116",
                              "relay_server": c["bind_ip"] + ":21117",
                              "api_server": "", "key": key}, indent=2))
        return 0
    except (OSError, ValueError, KeyError, binascii.Error, subprocess.CalledProcessError) as exc:
        print(f"serverctl: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
