#!/usr/bin/env python3

from __future__ import annotations

import array
import json
import os
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from dataclasses import dataclass
from pathlib import Path

from wayland_capture import (
    CaptureError,
    WL_DISPLAY_ID,
    capture_output,
    pack_i32,
    pack_string,
    pack_u32,
    parse_string,
    parse_u32,
)


KEYMAP_FORMAT_XKB_V1 = 1
XKB_KEYCODE_OFFSET = 8
BTN_LEFT = 0x110
BTN_RIGHT = 0x111
BTN_MIDDLE = 0x112
POINTER_BUTTON_PRESSED = 1
POINTER_BUTTON_RELEASED = 0
KEY_PRESSED = 1
KEY_RELEASED = 0

KEYSYM_TO_EVDEV = {
    "escape": 1,
    "1": 2,
    "2": 3,
    "3": 4,
    "4": 5,
    "5": 6,
    "6": 7,
    "7": 8,
    "8": 9,
    "9": 10,
    "0": 11,
    "minus": 12,
    "equal": 13,
    "backspace": 14,
    "tab": 15,
    "q": 16,
    "w": 17,
    "e": 18,
    "r": 19,
    "t": 20,
    "y": 21,
    "u": 22,
    "i": 23,
    "o": 24,
    "p": 25,
    "bracketleft": 26,
    "bracketright": 27,
    "return": 28,
    "control_l": 29,
    "a": 30,
    "s": 31,
    "d": 32,
    "f": 33,
    "g": 34,
    "h": 35,
    "j": 36,
    "k": 37,
    "l": 38,
    "semicolon": 39,
    "apostrophe": 40,
    "grave": 41,
    "shift_l": 42,
    "backslash": 43,
    "z": 44,
    "x": 45,
    "c": 46,
    "v": 47,
    "b": 48,
    "n": 49,
    "m": 50,
    "comma": 51,
    "period": 52,
    "slash": 53,
    "shift_r": 54,
    "alt_l": 56,
    "space": 57,
    "caps_lock": 58,
    "f1": 59,
    "f2": 60,
    "f3": 61,
    "f4": 62,
    "f5": 63,
    "f6": 64,
    "f7": 65,
    "f8": 66,
    "f9": 67,
    "f10": 68,
    "home": 102,
    "up": 103,
    "prior": 104,
    "left": 105,
    "right": 106,
    "end": 107,
    "down": 108,
    "next": 109,
    "insert": 110,
    "delete": 111,
    "control_r": 97,
    "alt_r": 100,
    "super_l": 125,
    "super_r": 126,
}

BROWSER_CODE_TO_EVDEV = {
    "Escape": 1,
    "Digit1": 2,
    "Digit2": 3,
    "Digit3": 4,
    "Digit4": 5,
    "Digit5": 6,
    "Digit6": 7,
    "Digit7": 8,
    "Digit8": 9,
    "Digit9": 10,
    "Digit0": 11,
    "Minus": 12,
    "Equal": 13,
    "Backspace": 14,
    "Tab": 15,
    "KeyQ": 16,
    "KeyW": 17,
    "KeyE": 18,
    "KeyR": 19,
    "KeyT": 20,
    "KeyY": 21,
    "KeyU": 22,
    "KeyI": 23,
    "KeyO": 24,
    "KeyP": 25,
    "BracketLeft": 26,
    "BracketRight": 27,
    "Enter": 28,
    "ControlLeft": 29,
    "KeyA": 30,
    "KeyS": 31,
    "KeyD": 32,
    "KeyF": 33,
    "KeyG": 34,
    "KeyH": 35,
    "KeyJ": 36,
    "KeyK": 37,
    "KeyL": 38,
    "Semicolon": 39,
    "Quote": 40,
    "Backquote": 41,
    "ShiftLeft": 42,
    "Backslash": 43,
    "KeyZ": 44,
    "KeyX": 45,
    "KeyC": 46,
    "KeyV": 47,
    "KeyB": 48,
    "KeyN": 49,
    "KeyM": 50,
    "Comma": 51,
    "Period": 52,
    "Slash": 53,
    "ShiftRight": 54,
    "AltLeft": 56,
    "Space": 57,
    "CapsLock": 58,
    "F1": 59,
    "F2": 60,
    "F3": 61,
    "F4": 62,
    "F5": 63,
    "F6": 64,
    "F7": 65,
    "F8": 66,
    "F9": 67,
    "F10": 68,
    "ControlRight": 97,
    "AltRight": 100,
    "Home": 102,
    "ArrowUp": 103,
    "PageUp": 104,
    "ArrowLeft": 105,
    "ArrowRight": 106,
    "End": 107,
    "ArrowDown": 108,
    "PageDown": 109,
    "Insert": 110,
    "Delete": 111,
    "MetaLeft": 125,
    "MetaRight": 126,
}

MODIFIER_KEYCODES = {29, 42, 54, 56, 97, 100, 125, 126}


class InteractiveError(RuntimeError):
    pass


class InteractiveHTTPServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def handle_error(self, request, client_address) -> None:  # type: ignore[override]
        exc_type, exc, _ = sys.exc_info()
        if isinstance(exc, (BrokenPipeError, ConnectionResetError, ConnectionAbortedError)):
            return
        super().handle_error(request, client_address)


LOG_PATH: Path | None = Path(os.environ["MACLAND_INTERACTIVE_LOG"]) if "MACLAND_INTERACTIVE_LOG" in os.environ else None


def log_event(message: str) -> None:
    if LOG_PATH is None:
        return
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a") as handle:
        handle.write(f"{time.time():.3f} {message}\n")


@dataclass
class Globals:
    seat_name: int | None = None
    output_name: int | None = None
    pointer_manager_name: int | None = None
    pointer_manager_version: int = 0
    keyboard_manager_name: int | None = None
    keyboard_manager_version: int = 0


class WaylandSocket:
    def __init__(self, runtime_dir: str, display_name: str) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(os.path.join(runtime_dir, display_name))
        self.buffer = bytearray()
        self.next_id = 2
        self.interfaces: dict[int, str] = {WL_DISPLAY_ID: "wl_display"}

    def close(self) -> None:
        self.sock.close()

    def new_id(self, interface: str) -> int:
        object_id = self.next_id
        self.next_id += 1
        self.interfaces[object_id] = interface
        return object_id

    def destroy_id(self, object_id: int) -> None:
        self.interfaces.pop(object_id, None)

    def send(self, object_id: int, opcode: int, payload: bytes = b"", fds: list[int] | None = None) -> None:
        size = 8 + len(payload)
        header = struct.pack("<II", object_id, (size << 16) | opcode)
        ancillary = []
        if fds:
            fd_array = array.array("i", fds)
            ancillary.append((socket.SOL_SOCKET, socket.SCM_RIGHTS, fd_array))
        self.sock.sendmsg([header, payload], ancillary)

    def recv(self, timeout: float | None = None) -> tuple[int, str, int, bytes]:
        if timeout is not None:
            self.sock.settimeout(timeout)
        try:
            while len(self.buffer) < 8:
                data, _, _, _ = self.sock.recvmsg(65536, 0)
                if not data:
                    raise InteractiveError("wayland socket closed")
                self.buffer.extend(data)
            object_id, size_opcode = struct.unpack_from("<II", self.buffer, 0)
            size = size_opcode >> 16
            opcode = size_opcode & 0xFFFF
            while len(self.buffer) < size:
                data, _, _, _ = self.sock.recvmsg(65536, 0)
                if not data:
                    raise InteractiveError("wayland socket closed mid-message")
                self.buffer.extend(data)
            payload = bytes(self.buffer[8:size])
            del self.buffer[:size]
            return object_id, self.interfaces.get(object_id, "unknown"), opcode, payload
        finally:
            if timeout is not None:
                self.sock.settimeout(None)


def bind(conn: WaylandSocket, registry_id: int, name: int, interface_name: str, version: int) -> int:
    object_id = conn.new_id(interface_name)
    payload = pack_u32(name) + pack_string(interface_name) + pack_u32(version) + pack_u32(object_id)
    conn.send(registry_id, 0, payload)
    return object_id


def roundtrip(conn: WaylandSocket) -> None:
    callback_id = conn.new_id("wl_callback")
    conn.send(WL_DISPLAY_ID, 0, pack_u32(callback_id))
    while True:
        object_id, interface, opcode, payload = conn.recv(timeout=5)
        if interface == "wl_callback" and object_id == callback_id and opcode == 0:
            conn.destroy_id(callback_id)
            return
        if interface == "wl_display" and opcode == 0:
            raise InteractiveError(f"wl_display error: {payload!r}")


def discover_globals(conn: WaylandSocket) -> tuple[int, Globals]:
    registry_id = conn.new_id("wl_registry")
    callback_id = conn.new_id("wl_callback")
    conn.send(WL_DISPLAY_ID, 1, pack_u32(registry_id))
    conn.send(WL_DISPLAY_ID, 0, pack_u32(callback_id))

    globals_found = Globals()
    while True:
        object_id, interface, opcode, payload = conn.recv(timeout=5)
        if interface == "wl_registry" and opcode == 0:
            name, offset = parse_u32(payload, 0)
            iface_name, offset = parse_string(payload, offset)
            version, _ = parse_u32(payload, offset)
            if iface_name == "wl_seat" and globals_found.seat_name is None:
                globals_found.seat_name = name
            elif iface_name == "wl_output" and globals_found.output_name is None:
                globals_found.output_name = name
            elif iface_name == "zwlr_virtual_pointer_manager_v1" and globals_found.pointer_manager_name is None:
                globals_found.pointer_manager_name = name
                globals_found.pointer_manager_version = version
            elif iface_name == "zwp_virtual_keyboard_manager_v1" and globals_found.keyboard_manager_name is None:
                globals_found.keyboard_manager_name = name
                globals_found.keyboard_manager_version = version
        elif interface == "wl_callback" and object_id == callback_id and opcode == 0:
            conn.destroy_id(callback_id)
            break
        elif interface == "wl_display" and opcode == 0:
            raise InteractiveError(f"wl_display error: {payload!r}")
    return registry_id, globals_found


def default_keymap_text() -> str:
    return """
xkb_keymap {
    xkb_keycodes  { include "evdev+aliases(qwerty)" };
    xkb_types     { include "complete" };
    xkb_compat    { include "complete" };
    xkb_symbols   { include "pc+us+inet(evdev)" };
    xkb_geometry  { include "pc(pc105)" };
};
""".strip()


class InputSession:
    def __init__(self, runtime_dir: str, display_name: str) -> None:
        log_event("input.connect")
        self.conn = WaylandSocket(runtime_dir, display_name)
        registry_id, globals_found = discover_globals(self.conn)
        if globals_found.seat_name is None:
            raise InteractiveError("compositor does not expose wl_seat")
        if globals_found.pointer_manager_name is None:
            raise InteractiveError("compositor does not expose zwlr_virtual_pointer_manager_v1")
        if globals_found.keyboard_manager_name is None:
            raise InteractiveError("compositor does not expose zwp_virtual_keyboard_manager_v1")
        seat_id = bind(self.conn, registry_id, globals_found.seat_name, "wl_seat", 7)
        pointer_manager_id = bind(
            self.conn,
            registry_id,
            globals_found.pointer_manager_name,
            "zwlr_virtual_pointer_manager_v1",
            min(2, globals_found.pointer_manager_version),
        )
        keyboard_manager_id = bind(
            self.conn,
            registry_id,
            globals_found.keyboard_manager_name,
            "zwp_virtual_keyboard_manager_v1",
            min(1, globals_found.keyboard_manager_version),
        )
        self.pointer_id = self.conn.new_id("zwlr_virtual_pointer_v1")
        self.conn.send(pointer_manager_id, 0, pack_u32(seat_id) + pack_u32(self.pointer_id))
        self.keyboard_id = self.conn.new_id("zwp_virtual_keyboard_v1")
        self.conn.send(keyboard_manager_id, 0, pack_u32(seat_id) + pack_u32(self.keyboard_id))
        roundtrip(self.conn)
        log_event("input.keymap")
        self._install_keymap(default_keymap_text())
        self.active_modifiers: set[int] = set()
        log_event("input.ready")

    def close(self) -> None:
        try:
            self.conn.send(self.keyboard_id, 3)
            self.conn.send(self.pointer_id, 8)
        except Exception:
            pass
        self.conn.close()

    def _install_keymap(self, keymap_text: str) -> None:
        payload = keymap_text.encode("utf-8")
        fd, path = tempfile.mkstemp(prefix="macland-keymap-")
        try:
            os.write(fd, payload)
            os.lseek(fd, 0, os.SEEK_SET)
            self.conn.send(
                self.keyboard_id,
                0,
                pack_u32(KEYMAP_FORMAT_XKB_V1) + pack_u32(len(payload)),
                fds=[fd],
            )
            roundtrip(self.conn)
        finally:
            os.close(fd)
            os.unlink(path)

    def move_absolute(self, x: int, y: int, width: int, height: int) -> None:
        now = int(time.time() * 1000) & 0xFFFFFFFF
        self.conn.send(
            self.pointer_id,
            1,
            pack_u32(now) + pack_u32(max(0, x)) + pack_u32(max(0, y)) + pack_u32(max(1, width)) + pack_u32(max(1, height)),
        )
        self.conn.send(self.pointer_id, 4)

    def button(self, button: int, pressed: bool) -> None:
        now = int(time.time() * 1000) & 0xFFFFFFFF
        state = POINTER_BUTTON_PRESSED if pressed else POINTER_BUTTON_RELEASED
        self.conn.send(self.pointer_id, 2, pack_u32(now) + pack_u32(button) + pack_u32(state))
        self.conn.send(self.pointer_id, 4)

    def axis(self, axis: int, value: float, discrete: int) -> None:
        now = int(time.time() * 1000) & 0xFFFFFFFF
        fixed = int(value * 256)
        self.conn.send(self.pointer_id, 5, pack_u32(0))
        self.conn.send(self.pointer_id, 3, pack_u32(now) + pack_u32(axis) + pack_i32(fixed))
        self.conn.send(self.pointer_id, 7, pack_u32(now) + pack_u32(axis))
        self.conn.send(self.pointer_id, 8, pack_u32(now) + pack_u32(axis) + pack_i32(fixed) + pack_i32(discrete))
        self.conn.send(self.pointer_id, 4)

    def key(self, keycode: int, pressed: bool) -> None:
        now = int(time.time() * 1000) & 0xFFFFFFFF
        state = KEY_PRESSED if pressed else KEY_RELEASED
        xkb_keycode = keycode + XKB_KEYCODE_OFFSET
        self.conn.send(self.keyboard_id, 1, pack_u32(now) + pack_u32(xkb_keycode) + pack_u32(state))

    def sync_modifiers(self, desired_modifiers: set[int]) -> None:
        for keycode in sorted(self.active_modifiers - desired_modifiers):
            self.key(keycode, False)
        for keycode in sorted(desired_modifiers - self.active_modifiers):
            self.key(keycode, True)
        self.active_modifiers = set(desired_modifiers)


class InteractiveViewer:
    def __init__(self, image_path: Path, runtime_dir: str, display_name: str, title: str) -> None:
        log_event("viewer.init")
        self.image_path = image_path
        self.runtime_dir = runtime_dir
        self.display_name = display_name
        self.title = title
        self.stop_event = threading.Event()
        self.closed = False
        self.capture_failed: str | None = None
        self.input_failed: str | None = None
        self.input_session: InputSession | None = None
        try:
            log_event("capture.prefight.begin")
            capture_output(
                self.runtime_dir,
                self.display_name,
                self.image_path,
                None,
                include_demo_surface=False,
            )
            log_event("capture.prefight.ready")
        except Exception as err:
            self.capture_failed = str(err)
            log_event(f"capture.prefight.error {err}")
        log_event("viewer.server.begin")
        self.server = InteractiveHTTPServer(("127.0.0.1", 0), self.make_handler())
        self.server.timeout = 0.5
        self.base_url = f"http://127.0.0.1:{self.server.server_address[1]}"
        log_event(f"viewer.server.ready {self.base_url}")
        self.capture_thread = threading.Thread(target=self.capture_loop, daemon=True)
        self.capture_thread.start()
        log_event("capture.thread.started")
        self.input_thread = threading.Thread(target=self.init_input_session, daemon=True)
        self.input_thread.start()
        log_event("input.thread.started")

    def run(self) -> int:
        signal.signal(signal.SIGTERM, lambda *_args: self.close())
        signal.signal(signal.SIGINT, lambda *_args: self.close())
        subprocess.Popen(["open", self.base_url])
        log_event("viewer.browser.opened")
        while not self.stop_event.is_set():
            self.server.handle_request()
        return 0

    def close(self) -> None:
        if self.closed:
            return
        self.closed = True
        log_event("viewer.close")
        self.stop_event.set()
        try:
            if self.input_session is not None:
                self.input_session.close()
        except Exception:
            pass
        try:
            self.server.server_close()
        except Exception:
            pass
        current = threading.current_thread()
        for thread in (getattr(self, "capture_thread", None), getattr(self, "input_thread", None)):
            if thread is not None and thread.is_alive() and thread is not current:
                thread.join(timeout=1)

    def init_input_session(self) -> None:
        try:
            self.input_session = InputSession(self.runtime_dir, self.display_name)
            self.input_failed = None
        except Exception as err:
            self.input_failed = str(err)
            log_event(f"input.error {err}")

    def capture_loop(self) -> None:
        while not self.stop_event.is_set():
            try:
                capture_output(
                    self.runtime_dir,
                    self.display_name,
                    self.image_path,
                    None,
                    include_demo_surface=False,
                )
                self.capture_failed = None
                log_event("capture.loop.ready")
            except Exception as err:
                self.capture_failed = str(err)
                log_event(f"capture.loop.error {err}")
            time.sleep(0.016)

    def handle_motion(self, x: int, y: int, width: int, height: int) -> None:
        if self.input_session is None:
            return
        self.input_session.move_absolute(x, y, width, height)

    def handle_button(self, button: int, pressed: bool) -> None:
        if self.input_session is None:
            return
        self.input_session.button(button, pressed)

    def handle_mousewheel(self, delta: float) -> None:
        if self.input_session is None:
            return
        if delta:
            self.input_session.axis(1, float(-delta) / 120.0, int(-delta / 120))

    def handle_key(
        self,
        keysym: str,
        code: str,
        pressed: bool,
        modifiers: dict[str, bool] | None = None,
    ) -> None:
        if self.input_session is None:
            return
        desired_modifiers = modifier_keycodes_from_state(modifiers or {})
        keycode = map_key_event(keysym, code)
        if keycode is not None:
            if keycode not in MODIFIER_KEYCODES:
                self.input_session.sync_modifiers(desired_modifiers)
            log_event(f"input.key {code or keysym} -> {keycode + XKB_KEYCODE_OFFSET} {'down' if pressed else 'up'}")
            self.input_session.key(keycode, pressed)
            if keycode in MODIFIER_KEYCODES:
                if pressed:
                    self.input_session.active_modifiers.add(keycode)
                else:
                    self.input_session.active_modifiers.discard(keycode)

    def status_payload(self) -> dict[str, str | bool | None]:
        return {
            "capture_failed": self.capture_failed,
            "input_failed": self.input_failed,
            "frame_ready": self.image_path.exists(),
        }

    def html_page(self) -> bytes:
        title = self.title.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        return f"""<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>{title}</title>
  <style>
    html, body {{
      margin: 0;
      background: #101114;
      color: #f1f3f5;
      font-family: -apple-system, BlinkMacSystemFont, sans-serif;
      height: 100%;
    }}
    #shell {{
      display: grid;
      grid-template-rows: auto 1fr;
      height: 100%;
    }}
    #status {{
      padding: 10px 14px;
      background: #17191d;
      border-bottom: 1px solid #23262c;
      font-size: 13px;
    }}
    #stage {{
      display: flex;
      align-items: center;
      justify-content: center;
      outline: none;
      overflow: hidden;
      height: 100%;
    }}
    #screen {{
      max-width: 100%;
      max-height: 100%;
      image-rendering: auto;
      user-select: none;
      -webkit-user-drag: none;
    }}
  </style>
</head>
<body>
  <div id="shell">
    <div id="status">connecting… click the frame to focus keyboard input</div>
    <div id="stage" tabindex="0">
      <img id="screen" alt="macland frame" draggable="false">
    </div>
  </div>
  <script>
    const status = document.getElementById('status');
    const stage = document.getElementById('stage');
    const screen = document.getElementById('screen');
    let failedHeartbeats = 0;
    let closedForShutdown = false;
    function handleServerShutdown() {{
      if (closedForShutdown) return;
      closedForShutdown = true;
      status.textContent = 'macland host exited, closing tab…';
      setTimeout(() => {{
        window.close();
        setTimeout(() => {{
          document.body.innerHTML = '<div style="display:flex;align-items:center;justify-content:center;height:100vh;background:#101114;color:#f1f3f5;font-family:-apple-system,BlinkMacSystemFont,sans-serif;">macland host exited. You can close this tab.</div>';
        }}, 150);
      }}, 50);
    }}
    function refreshFrame() {{
      if (closedForShutdown) return;
      screen.src = '/frame.png?ts=' + Date.now();
    }}
    async function refreshStatus() {{
      try {{
        const response = await fetch('/status');
        failedHeartbeats = 0;
        const payload = await response.json();
        if (payload.capture_failed) {{
          status.textContent = 'capture: ' + payload.capture_failed;
        }} else if (payload.input_failed) {{
          status.textContent = 'frame live, input unavailable: ' + payload.input_failed;
        }} else {{
          status.textContent = payload.frame_ready ? 'frame live, input attached' : 'waiting for first frame…';
        }}
      }} catch (_error) {{
        failedHeartbeats += 1;
        if (failedHeartbeats >= 3) {{
          handleServerShutdown();
          return;
        }}
        status.textContent = 'waiting for host…';
      }}
    }}
    function send(kind, extra) {{
      if (kind === 'key') {{
        console.log('[macland:key]', extra.code, extra.keysym, extra.pressed ? 'down' : 'up', {{
          altKey: !!extra.altKey,
          ctrlKey: !!extra.ctrlKey,
          metaKey: !!extra.metaKey,
          shiftKey: !!extra.shiftKey
        }});
      }}
      fetch('/input', {{
        method: 'POST',
        headers: {{'Content-Type': 'application/json'}},
        body: JSON.stringify({{type: kind, ...extra}})
      }}).catch(() => {{}});
    }}
    function focusStage() {{
      stage.focus();
    }}
    window.addEventListener('load', focusStage);
    stage.addEventListener('click', focusStage);
    screen.addEventListener('mousemove', (event) => {{
      send('motion', {{x: event.offsetX, y: event.offsetY, width: screen.clientWidth, height: screen.clientHeight}});
    }});
    screen.addEventListener('mousedown', (event) => {{
      focusStage();
      const buttons = {{0: 'left', 1: 'middle', 2: 'right'}};
      send('button', {{button: buttons[event.button] || 'left', pressed: true}});
    }});
    screen.addEventListener('mouseup', (event) => {{
      const buttons = {{0: 'left', 1: 'middle', 2: 'right'}};
      send('button', {{button: buttons[event.button] || 'left', pressed: false}});
    }});
    screen.addEventListener('contextmenu', (event) => event.preventDefault());
    screen.addEventListener('wheel', (event) => {{
      event.preventDefault();
      send('wheel', {{delta: event.deltaY}});
    }}, {{passive: false}});
    window.addEventListener('keydown', (event) => {{
      if (event.repeat) return;
      event.preventDefault();
      send('key', {{
        keysym: event.key,
        code: event.code,
        pressed: true,
        altKey: event.altKey,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey
      }});
    }});
    window.addEventListener('keyup', (event) => {{
      event.preventDefault();
      send('key', {{
        keysym: event.key,
        code: event.code,
        pressed: false,
        altKey: event.altKey,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey
      }});
    }});
    refreshFrame();
    refreshStatus();
    setInterval(refreshFrame, 16);
    setInterval(refreshStatus, 200);
  </script>
</body>
</html>""".encode("utf-8")

    def make_handler(self):
        host = self

        class Handler(BaseHTTPRequestHandler):
            def safe_write(self, payload: bytes) -> None:
                try:
                    self.wfile.write(payload)
                except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                    return

            def do_GET(self):
                if self.path.startswith("/frame.png"):
                    if not host.image_path.exists():
                        self.send_response(503)
                        self.end_headers()
                        return
                    payload = host.image_path.read_bytes()
                    self.send_response(200)
                    self.send_header("Content-Type", "image/png")
                    self.send_header("Content-Length", str(len(payload)))
                    self.send_header("Cache-Control", "no-store")
                    self.end_headers()
                    self.safe_write(payload)
                    return
                if self.path == "/status":
                    payload = json.dumps(host.status_payload()).encode("utf-8")
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(payload)))
                    self.end_headers()
                    self.safe_write(payload)
                    return
                payload = host.html_page()
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.safe_write(payload)

            def do_POST(self):
                if self.path != "/input":
                    self.send_response(404)
                    self.end_headers()
                    return
                length = int(self.headers.get("Content-Length", "0"))
                payload = json.loads(self.rfile.read(length) or b"{}")
                event_type = payload.get("type")
                if event_type == "motion":
                    host.handle_motion(
                        int(payload.get("x", 0)),
                        int(payload.get("y", 0)),
                        int(payload.get("width", 1)),
                        int(payload.get("height", 1)),
                    )
                elif event_type == "button":
                    buttons = {"left": BTN_LEFT, "middle": BTN_MIDDLE, "right": BTN_RIGHT}
                    host.handle_button(buttons.get(str(payload.get("button", "left")), BTN_LEFT), bool(payload.get("pressed")))
                elif event_type == "wheel":
                    host.handle_mousewheel(float(payload.get("delta", 0)))
                elif event_type == "key":
                    host.handle_key(
                        str(payload.get("keysym", "")),
                        str(payload.get("code", "")),
                        bool(payload.get("pressed")),
                        {
                            "altKey": bool(payload.get("altKey")),
                            "ctrlKey": bool(payload.get("ctrlKey")),
                            "metaKey": bool(payload.get("metaKey")),
                            "shiftKey": bool(payload.get("shiftKey")),
                        },
                    )
                self.send_response(204)
                self.end_headers()

            def log_message(self, _format, *args):
                return

        return Handler


def map_keysym(keysym: str) -> int | None:
    normalized = keysym.lower()
    if len(normalized) == 1 and normalized.isalpha():
        return KEYSYM_TO_EVDEV.get(normalized)
    aliases = {
        "enter": "return",
        "arrowup": "up",
        "arrowdown": "down",
        "arrowleft": "left",
        "arrowright": "right",
        "prior": "prior",
        "next": "next",
        "page_up": "prior",
        "page_down": "next",
        "pageup": "prior",
        "pagedown": "next",
        "command": "super_l",
        "command_l": "super_l",
        "command_r": "super_r",
        "meta": "super_l",
        "option": "alt_l",
        "option_l": "alt_l",
        "option_r": "alt_r",
        "alt": "alt_l",
        "control": "control_l",
        "shift": "shift_l",
        " ": "space",
    }
    return KEYSYM_TO_EVDEV.get(aliases.get(normalized, normalized))


def map_key_event(keysym: str, code: str) -> int | None:
    if code:
        mapped = BROWSER_CODE_TO_EVDEV.get(code)
        if mapped is not None:
            return mapped
    return map_keysym(keysym)


def modifier_keycodes_from_state(modifiers: dict[str, bool]) -> set[int]:
    result: set[int] = set()
    if modifiers.get("altKey"):
        result.add(56)
    if modifiers.get("ctrlKey"):
        result.add(29)
    if modifiers.get("metaKey"):
        result.add(125)
    if modifiers.get("shiftKey"):
        result.add(42)
    return result


def main(argv: list[str]) -> int:
    log_event("main.entry")
    if len(argv) != 5:
        print(
            "usage: wayland_interactive.py <runtime-dir> <wayland-display> <image.png> <title>",
            file=sys.stderr,
        )
        return 1
    runtime_dir = argv[1]
    display_name = argv[2]
    image_path = Path(argv[3]).resolve()
    title = argv[4]
    try:
        viewer = InteractiveViewer(image_path, runtime_dir, display_name, title)
        return viewer.run()
    except (InteractiveError, CaptureError) as err:
        print(f"error: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
