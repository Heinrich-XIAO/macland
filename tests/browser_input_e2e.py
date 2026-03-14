#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import time
import unittest
import urllib.request
from pathlib import Path


ROOT = Path("/Users/heinrich/Documents/macland")
SWAY_LOG = ROOT / "repos" / "sway" / "artifacts" / "run" / "interactive-host.log"


class BrowserInputE2E(unittest.TestCase):
    def setUp(self) -> None:
        if SWAY_LOG.exists():
            SWAY_LOG.unlink()

    def tearDown(self) -> None:
        if SWAY_LOG.exists():
            SWAY_LOG.unlink()

    def test_sway_browser_host_accepts_modifier_shortcuts(self) -> None:
        process = subprocess.Popen(
            ["./macland", "run", "sway", "--execute"],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        try:
            url = self.wait_for_server_url()
            self.post_input(
                url,
                {"type": "key", "keysym": "Alt", "code": "AltLeft", "pressed": True},
            )
            self.post_input(
                url,
                {"type": "key", "keysym": "Enter", "code": "Enter", "pressed": True},
            )
            self.post_input(
                url,
                {"type": "key", "keysym": "Enter", "code": "Enter", "pressed": False},
            )
            self.post_input(
                url,
                {"type": "key", "keysym": "Alt", "code": "AltLeft", "pressed": False},
            )

            log_text = self.wait_for_log_lines(
                [
                    "input.key AltLeft -> 64 down",
                    "input.key Enter -> 36 down",
                    "input.key Enter -> 36 up",
                    "input.key AltLeft -> 64 up",
                ]
            )
            self.assertIn("input.ready", log_text)
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
            stdout, _ = process.communicate(timeout=20)
            self.assertEqual(process.returncode, 130, stdout)

    def wait_for_server_url(self) -> str:
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if SWAY_LOG.exists():
                text = SWAY_LOG.read_text()
                match = re.search(r"viewer\.server\.ready (http://127\.0\.0\.1:\d+)", text)
                if match:
                    return match.group(1)
            time.sleep(0.1)
        self.fail("timed out waiting for sway interactive host URL")

    def post_input(self, url: str, payload: dict[str, object]) -> None:
        request = urllib.request.Request(
            url + "/input",
            data=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=5) as response:
            self.assertEqual(response.status, 204)

    def wait_for_log_lines(self, expected_lines: list[str]) -> str:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if SWAY_LOG.exists():
                text = SWAY_LOG.read_text()
                if all(line in text for line in expected_lines):
                    return text
            time.sleep(0.1)
        current = SWAY_LOG.read_text() if SWAY_LOG.exists() else "<missing log>"
        self.fail(f"timed out waiting for expected key log lines\n{current}")


if __name__ == "__main__":
    unittest.main()
