import base64
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import serverctl

IMAGE = "ghcr.io/rustdesk/rustdesk-server@sha256:" + "a" * 64


class ServerConfigTests(unittest.TestCase):
    def test_private_addresses(self):
        for ip in ("10.0.0.1", "172.16.0.1", "192.168.1.2", "100.64.0.5"):
            self.assertEqual(serverctl.validate(ip, IMAGE)["bind_ip"], ip)

    def test_reject_public_and_wildcard(self):
        for ip in ("0.0.0.0", "8.8.8.8", "127.0.0.1", "::", "example.com"):
            with self.subTest(ip=ip), self.assertRaises(ValueError):
                serverctl.validate(ip, IMAGE)

    def test_reject_mutable_or_other_image(self):
        for image in ("ghcr.io/rustdesk/rustdesk-server:latest", "other@sha256:" + "a" * 64):
            with self.assertRaises(ValueError):
                serverctl.validate("10.0.0.1", image)

    def test_configure_does_not_overwrite(self):
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            config = state / "config.json"
            with patch.object(serverctl, "STATE", state), patch.object(serverctl, "CONFIG", config):
                serverctl.configure("10.0.0.1", IMAGE)
                self.assertEqual(config.stat().st_mode & 0o777, 0o600)
                self.assertEqual(state.stat().st_mode & 0o777, 0o700)
                with self.assertRaises(FileExistsError):
                    serverctl.configure("10.0.0.2", IMAGE)
                self.assertEqual(serverctl.load_config()["bind_ip"], "10.0.0.1")

    def test_environment_cannot_override_bind_or_image(self):
        c = serverctl.validate("10.0.0.1", IMAGE)
        with patch.dict(os.environ, {"RUSTDESKV_BIND_IP": "0.0.0.0", "RUSTDESKV_SERVER_IMAGE": "other"}):
            command, env = serverctl.compose_args(c, ("config", "--quiet"))
        self.assertEqual(env["RUSTDESKV_BIND_IP"], "10.0.0.1")
        self.assertEqual(env["RUSTDESKV_SERVER_IMAGE"], IMAGE)
        self.assertEqual(command[-2:], ["config", "--quiet"])

    def test_public_key(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / "key.pub"
            expected = base64.b64encode(bytes(32)).decode()
            p.write_text(expected + "\n")
            self.assertEqual(serverctl.public_key(p), expected)
            p.write_text("wrong")
            with self.assertRaises(ValueError):
                serverctl.public_key(p)


if __name__ == "__main__":
    unittest.main()
