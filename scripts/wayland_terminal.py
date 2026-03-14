#!/usr/bin/env python3

from __future__ import annotations

import mmap
import os
import pty
import select
import signal
import sys
import termios
import time
from dataclasses import dataclass
from pathlib import Path

from wayland_capture import (
    WL_DISPLAY_ID,
    WL_SHM_FORMAT_XRGB8888,
    WaylandSocket,
    bind,
    create_shm_buffer,
    pack_i32,
    pack_string,
    pack_u32,
    parse_string,
    parse_u32,
)

LOG_PATH = os.environ.get("MACLAND_TERM_LOG")


def log(message: str) -> None:
    if not LOG_PATH:
        return
    try:
        with open(LOG_PATH, "a", encoding="utf-8") as handle:
            handle.write(message + "\n")
    except OSError:
        return

@dataclass
class Globals:
    shm_name: int | None = None
    compositor_name: int | None = None
    compositor_version: int = 0
    xdg_wm_base_name: int | None = None
    xdg_wm_base_version: int = 0
    seat_name: int | None = None


FONT8X8_BASIC = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],  # 0x20
    [0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18, 0x00],  # 0x21
    [0x36, 0x36, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00],  # 0x22
    [0x36, 0x36, 0x7F, 0x36, 0x7F, 0x36, 0x36, 0x00],  # 0x23
    [0x0C, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x0C, 0x00],  # 0x24
    [0x00, 0x63, 0x33, 0x18, 0x0C, 0x66, 0x63, 0x00],  # 0x25
    [0x1C, 0x36, 0x1C, 0x6E, 0x3B, 0x33, 0x6E, 0x00],  # 0x26
    [0x06, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00],  # 0x27
    [0x18, 0x0C, 0x06, 0x06, 0x06, 0x0C, 0x18, 0x00],  # 0x28
    [0x06, 0x0C, 0x18, 0x18, 0x18, 0x0C, 0x06, 0x00],  # 0x29
    [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],  # 0x2A
    [0x00, 0x0C, 0x0C, 0x3F, 0x0C, 0x0C, 0x00, 0x00],  # 0x2B
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x06],  # 0x2C
    [0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x00],  # 0x2D
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00],  # 0x2E
    [0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x01, 0x00],  # 0x2F
    [0x3E, 0x63, 0x73, 0x7B, 0x6F, 0x67, 0x3E, 0x00],  # 0x30
    [0x0C, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F, 0x00],  # 0x31
    [0x1E, 0x33, 0x30, 0x1C, 0x06, 0x33, 0x3F, 0x00],  # 0x32
    [0x1E, 0x33, 0x30, 0x1C, 0x30, 0x33, 0x1E, 0x00],  # 0x33
    [0x38, 0x3C, 0x36, 0x33, 0x7F, 0x30, 0x78, 0x00],  # 0x34
    [0x3F, 0x03, 0x1F, 0x30, 0x30, 0x33, 0x1E, 0x00],  # 0x35
    [0x1C, 0x06, 0x03, 0x1F, 0x33, 0x33, 0x1E, 0x00],  # 0x36
    [0x3F, 0x33, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x00],  # 0x37
    [0x1E, 0x33, 0x33, 0x1E, 0x33, 0x33, 0x1E, 0x00],  # 0x38
    [0x1E, 0x33, 0x33, 0x3E, 0x30, 0x18, 0x0E, 0x00],  # 0x39
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x00],  # 0x3A
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x06],  # 0x3B
    [0x18, 0x0C, 0x06, 0x03, 0x06, 0x0C, 0x18, 0x00],  # 0x3C
    [0x00, 0x00, 0x3F, 0x00, 0x00, 0x3F, 0x00, 0x00],  # 0x3D
    [0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00],  # 0x3E
    [0x1E, 0x33, 0x30, 0x18, 0x0C, 0x00, 0x0C, 0x00],  # 0x3F
    [0x3E, 0x63, 0x7B, 0x7B, 0x7B, 0x03, 0x1E, 0x00],  # 0x40
    [0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x00],  # 0x41
    [0x3F, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3F, 0x00],  # 0x42
    [0x3C, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3C, 0x00],  # 0x43
    [0x1F, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1F, 0x00],  # 0x44
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x46, 0x7F, 0x00],  # 0x45
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00],  # 0x46
    [0x3C, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7C, 0x00],  # 0x47
    [0x33, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x33, 0x00],  # 0x48
    [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],  # 0x49
    [0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x00],  # 0x4A
    [0x67, 0x66, 0x36, 0x1E, 0x36, 0x66, 0x67, 0x00],  # 0x4B
    [0x0F, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7F, 0x00],  # 0x4C
    [0x63, 0x77, 0x7F, 0x7F, 0x6B, 0x63, 0x63, 0x00],  # 0x4D
    [0x63, 0x67, 0x6F, 0x7B, 0x73, 0x63, 0x63, 0x00],  # 0x4E
    [0x1C, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1C, 0x00],  # 0x4F
    [0x3F, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x0F, 0x00],  # 0x50
    [0x1E, 0x33, 0x33, 0x33, 0x3B, 0x1E, 0x38, 0x00],  # 0x51
    [0x3F, 0x66, 0x66, 0x3E, 0x36, 0x66, 0x67, 0x00],  # 0x52
    [0x1E, 0x33, 0x07, 0x0E, 0x38, 0x33, 0x1E, 0x00],  # 0x53
    [0x3F, 0x2D, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],  # 0x54
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x00],  # 0x55
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00],  # 0x56
    [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00],  # 0x57
    [0x63, 0x63, 0x36, 0x1C, 0x1C, 0x36, 0x63, 0x00],  # 0x58
    [0x33, 0x33, 0x33, 0x1E, 0x0C, 0x0C, 0x1E, 0x00],  # 0x59
    [0x7F, 0x63, 0x31, 0x18, 0x4C, 0x66, 0x7F, 0x00],  # 0x5A
    [0x1E, 0x06, 0x06, 0x06, 0x06, 0x06, 0x1E, 0x00],  # 0x5B
    [0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00],  # 0x5C
    [0x1E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1E, 0x00],  # 0x5D
    [0x08, 0x1C, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00],  # 0x5E
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],  # 0x5F
    [0x0C, 0x0C, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00],  # 0x60
    [0x00, 0x00, 0x1E, 0x30, 0x3E, 0x33, 0x6E, 0x00],  # 0x61
    [0x07, 0x06, 0x06, 0x3E, 0x66, 0x66, 0x3B, 0x00],  # 0x62
    [0x00, 0x00, 0x1E, 0x33, 0x03, 0x33, 0x1E, 0x00],  # 0x63
    [0x38, 0x30, 0x30, 0x3E, 0x33, 0x33, 0x6E, 0x00],  # 0x64
    [0x00, 0x00, 0x1E, 0x33, 0x3F, 0x03, 0x1E, 0x00],  # 0x65
    [0x1C, 0x36, 0x06, 0x0F, 0x06, 0x06, 0x0F, 0x00],  # 0x66
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x1F],  # 0x67
    [0x07, 0x06, 0x36, 0x6E, 0x66, 0x66, 0x67, 0x00],  # 0x68
    [0x0C, 0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],  # 0x69
    [0x30, 0x00, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E],  # 0x6A
    [0x07, 0x06, 0x66, 0x36, 0x1E, 0x36, 0x67, 0x00],  # 0x6B
    [0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],  # 0x6C
    [0x00, 0x00, 0x33, 0x7F, 0x7F, 0x6B, 0x63, 0x00],  # 0x6D
    [0x00, 0x00, 0x1B, 0x37, 0x33, 0x33, 0x33, 0x00],  # 0x6E
    [0x00, 0x00, 0x1E, 0x33, 0x33, 0x33, 0x1E, 0x00],  # 0x6F
    [0x00, 0x00, 0x3B, 0x66, 0x66, 0x3E, 0x06, 0x0F],  # 0x70
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x78],  # 0x71
    [0x00, 0x00, 0x3B, 0x6E, 0x66, 0x06, 0x0F, 0x00],  # 0x72
    [0x00, 0x00, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x00],  # 0x73
    [0x08, 0x0C, 0x3E, 0x0C, 0x0C, 0x2C, 0x18, 0x00],  # 0x74
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x33, 0x6E, 0x00],  # 0x75
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00],  # 0x76
    [0x00, 0x00, 0x63, 0x6B, 0x7F, 0x7F, 0x36, 0x00],  # 0x77
    [0x00, 0x00, 0x63, 0x36, 0x1C, 0x36, 0x63, 0x00],  # 0x78
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x3E, 0x30, 0x1F],  # 0x79
    [0x00, 0x00, 0x3F, 0x19, 0x0C, 0x26, 0x3F, 0x00],  # 0x7A
    [0x38, 0x0C, 0x0C, 0x07, 0x0C, 0x0C, 0x38, 0x00],  # 0x7B
    [0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x18, 0x00],  # 0x7C
    [0x07, 0x0C, 0x0C, 0x38, 0x0C, 0x0C, 0x07, 0x00],  # 0x7D
    [0x6E, 0x3B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],  # 0x7E
]

SHIFTED = {
    "1": "!",
    "2": "@",
    "3": "#",
    "4": "$",
    "5": "%",
    "6": "^",
    "7": "&",
    "8": "*",
    "9": "(",
    "0": ")",
    "-": "_",
    "=": "+",
    "[": "{",
    "]": "}",
    "\\": "|",
    ";": ":",
    "'": "\"",
    ",": "<",
    ".": ">",
    "/": "?",
    "`": "~",
}

EVDEV_TO_CHAR = {
    2: "1",
    3: "2",
    4: "3",
    5: "4",
    6: "5",
    7: "6",
    8: "7",
    9: "8",
    10: "9",
    11: "0",
    12: "-",
    13: "=",
    14: "\b",
    15: "\t",
    16: "q",
    17: "w",
    18: "e",
    19: "r",
    20: "t",
    21: "y",
    22: "u",
    23: "i",
    24: "o",
    25: "p",
    26: "[",
    27: "]",
    28: "\n",
    30: "a",
    31: "s",
    32: "d",
    33: "f",
    34: "g",
    35: "h",
    36: "j",
    37: "k",
    38: "l",
    39: ";",
    40: "'",
    41: "`",
    43: "\\",
    44: "z",
    45: "x",
    46: "c",
    47: "v",
    48: "b",
    49: "n",
    50: "m",
    51: ",",
    52: ".",
    53: "/",
    57: " ",
}


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
            if iface_name == "wl_shm" and globals_found.shm_name is None:
                globals_found.shm_name = name
            elif iface_name == "wl_compositor" and globals_found.compositor_name is None:
                globals_found.compositor_name = name
                globals_found.compositor_version = version
            elif iface_name == "xdg_wm_base" and globals_found.xdg_wm_base_name is None:
                globals_found.xdg_wm_base_name = name
                globals_found.xdg_wm_base_version = version
            elif iface_name == "wl_seat" and globals_found.seat_name is None:
                globals_found.seat_name = name
        elif interface == "wl_callback" and object_id == callback_id and opcode == 0:
            conn.destroy_id(callback_id)
            break
        elif interface == "wl_display" and opcode == 0:
            raise RuntimeError(f"wl_display error: {payload!r}")
    return registry_id, globals_found


def draw_text(buffer: mmap.mmap, cols: int, rows: int, cursor: tuple[int, int], title: str) -> None:
    width = cols * 8
    height = rows * 16
    bg = (0x16, 0x18, 0x20, 0xFF)
    fg = (0xE5, 0xE9, 0xF0, 0xFF)
    cursor_color = (0x88, 0xC0, 0xD0, 0xFF)
    pixels = bytearray(width * height * 4)
    for i in range(0, len(pixels), 4):
        pixels[i : i + 4] = bg
    lines = SCREEN.buffer[: rows]
    for y, line in enumerate(lines):
        for x, ch in enumerate(line[:cols]):
            draw_glyph(pixels, width, x * 8, y * 16, ch, fg)
    if title:
        draw_status(pixels, width, cols, title)
    cx, cy = cursor
    if 0 <= cx < cols and 0 <= cy < rows:
        draw_cursor(pixels, width, cx * 8, cy * 16, cursor_color)
    buffer.seek(0)
    buffer.write(pixels)


def draw_glyph(pixels: bytearray, width: int, x0: int, y0: int, ch: str, color: tuple[int, int, int, int]) -> None:
    idx = ord(ch)
    if idx < 32 or idx > 126:
        return
    bitmap = FONT8X8_BASIC[idx - 32]
    for row in range(8):
        bits = bitmap[row]
        for col in range(8):
            if bits & (1 << col):
                paint(pixels, width, x0 + col, y0 + row * 2, color)
                paint(pixels, width, x0 + col, y0 + row * 2 + 1, color)


def draw_cursor(pixels: bytearray, width: int, x0: int, y0: int, color: tuple[int, int, int, int]) -> None:
    for y in range(y0 + 14, y0 + 16):
        for x in range(x0, x0 + 8):
            paint(pixels, width, x, y, color)


def draw_status(pixels: bytearray, width: int, cols: int, title: str) -> None:
    status = f"{title}  (macland-term)"
    for x, ch in enumerate(status[:cols]):
        draw_glyph(pixels, width, x * 8, 0, ch, (0xA0, 0xA8, 0xB6, 0xFF))


def paint(pixels: bytearray, width: int, x: int, y: int, color: tuple[int, int, int, int]) -> None:
    idx = (y * width + x) * 4
    if 0 <= idx < len(pixels) - 3:
        b, g, r, a = color
        pixels[idx : idx + 4] = bytes((b, g, r, a))


class ScreenBuffer:
    def __init__(self, cols: int, rows: int) -> None:
        self.cols = cols
        self.rows = rows
        self.buffer = [[" " for _ in range(cols)] for _ in range(rows)]
        self.cursor_x = 0
        self.cursor_y = 1

    def write(self, data: bytes) -> None:
        for byte in data:
            ch = chr(byte)
            if ch == "\n":
                self.cursor_x = 0
                self.cursor_y += 1
                self.scroll_if_needed()
            elif ch == "\r":
                self.cursor_x = 0
            elif ch == "\b":
                if self.cursor_x > 0:
                    self.cursor_x -= 1
                    self.buffer[self.cursor_y][self.cursor_x] = " "
            elif ch == "\t":
                self.cursor_x = min(self.cols - 1, (self.cursor_x + 4) // 4 * 4)
            elif 32 <= byte < 127:
                self.buffer[self.cursor_y][self.cursor_x] = ch
                self.cursor_x += 1
                if self.cursor_x >= self.cols:
                    self.cursor_x = 0
                    self.cursor_y += 1
                    self.scroll_if_needed()

    def scroll_if_needed(self) -> None:
        if self.cursor_y < self.rows:
            return
        self.buffer.pop(1)
        self.buffer.append([" " for _ in range(self.cols)])
        self.cursor_y = self.rows - 1


SCREEN = ScreenBuffer(80, 24)


def spawn_shell() -> tuple[int, int]:
    master_fd, slave_fd = pty.openpty()
    pid = os.fork()
    if pid == 0:
        os.setsid()
        os.dup2(slave_fd, 0)
        os.dup2(slave_fd, 1)
        os.dup2(slave_fd, 2)
        os.close(master_fd)
        os.close(slave_fd)
        os.environ.setdefault("TERM", "xterm-256color")
        os.execl("/bin/zsh", "/bin/zsh")
    os.close(slave_fd)
    return master_fd, pid


def configure_pty(fd: int) -> None:
    attrs = termios.tcgetattr(fd)
    attrs[3] = attrs[3] & ~(termios.ECHO | termios.ICANON)
    termios.tcsetattr(fd, termios.TCSANOW, attrs)


def translate_key(keycode: int, shift: bool) -> bytes | None:
    if keycode not in EVDEV_TO_CHAR:
        return None
    ch = EVDEV_TO_CHAR[keycode]
    if ch == "\n":
        return b"\n"
    if ch == "\t":
        return b"\t"
    if ch == "\b":
        return b"\x7f"
    if shift:
        if "a" <= ch <= "z":
            ch = ch.upper()
        elif ch in SHIFTED:
            ch = SHIFTED[ch]
    return ch.encode("utf-8")


def main() -> int:
    log("macland-term start")
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR")
    display_name = os.environ.get("WAYLAND_DISPLAY")
    if not runtime_dir or not display_name:
        log("missing XDG_RUNTIME_DIR or WAYLAND_DISPLAY")
        print("XDG_RUNTIME_DIR and WAYLAND_DISPLAY are required", file=sys.stderr)
        return 1

    conn = WaylandSocket(runtime_dir, display_name)
    registry_id, globals_found = discover_globals(conn)
    if globals_found.shm_name is None or globals_found.compositor_name is None or globals_found.xdg_wm_base_name is None:
        raise RuntimeError("missing required globals")

    compositor_id = bind(
        conn,
        registry_id,
        globals_found.compositor_name,
        "wl_compositor",
        min(4, globals_found.compositor_version),
    )
    wm_base_id = bind(
        conn,
        registry_id,
        globals_found.xdg_wm_base_name,
        "xdg_wm_base",
        min(2, globals_found.xdg_wm_base_version),
    )
    shm_id = bind(conn, registry_id, globals_found.shm_name, "wl_shm", 1)
    seat_id = None
    if globals_found.seat_name is not None:
        seat_id = bind(conn, registry_id, globals_found.seat_name, "wl_seat", 7)

    surface_id = conn.new_id("wl_surface")
    conn.send(compositor_id, 0, pack_u32(surface_id))
    xdg_surface_id = conn.new_id("xdg_surface")
    conn.send(wm_base_id, 2, pack_u32(xdg_surface_id) + pack_u32(surface_id))
    toplevel_id = conn.new_id("xdg_toplevel")
    conn.send(xdg_surface_id, 1, pack_u32(toplevel_id))
    conn.send(toplevel_id, 2, pack_string("macland terminal"))
    conn.send(toplevel_id, 3, pack_string("macland.term"))

    keyboard_id = None

    conn.send(surface_id, 6)

    configured = False
    while not configured:
        object_id, interface, opcode, payload = conn.recv(timeout=5)
        if interface == "xdg_wm_base" and object_id == wm_base_id and opcode == 0:
            serial, _ = parse_u32(payload, 0)
            conn.send(wm_base_id, 3, pack_u32(serial))
        elif interface == "xdg_surface" and object_id == xdg_surface_id and opcode == 0:
            serial, _ = parse_u32(payload, 0)
            conn.send(xdg_surface_id, 4, pack_u32(serial))
            configured = True
            log("xdg_surface configured")
        elif interface == "wl_display" and opcode == 0:
            raise RuntimeError(f"wl_display error: {payload!r}")

    cols, rows = SCREEN.cols, SCREEN.rows
    width, height = cols * 8, rows * 16
    stride = width * 4
    shm_buffer = create_shm_buffer(conn, shm_id, width, height, stride, WL_SHM_FORMAT_XRGB8888, None, runtime_dir)
    buffer_map = mmap.mmap(shm_buffer.fd, stride * height, access=mmap.ACCESS_WRITE)

    master_fd, child_pid = spawn_shell()
    configure_pty(master_fd)
    shift_down = False
    running = True
    dirty = True
    title = "Shell: /bin/zsh"
    configured = True

    def stop(_signum, _frame) -> None:
        nonlocal running
        running = False

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)

    while running:
        rlist = [conn.sock, master_fd]
        readable, _, _ = select.select(rlist, [], [], 0.05)
        if conn.sock in readable:
            object_id, interface, opcode, payload = conn.recv(timeout=0)
            if interface == "xdg_wm_base" and opcode == 0:
                serial, _ = parse_u32(payload, 0)
                conn.send(object_id, 3, pack_u32(serial))
            elif interface == "xdg_surface" and object_id == xdg_surface_id and opcode == 0:
                serial, _ = parse_u32(payload, 0)
                conn.send(xdg_surface_id, 4, pack_u32(serial))
                configured = True
                log("xdg_surface configured")
            elif interface == "wl_seat" and object_id == seat_id and opcode == 0:
                capabilities, _ = parse_u32(payload, 0)
                if capabilities & 2 and keyboard_id is None:
                    keyboard_id = conn.new_id("wl_keyboard")
                    conn.send(seat_id, 1, pack_u32(keyboard_id))
            elif interface == "wl_keyboard" and object_id == keyboard_id and opcode == 3:
                _, offset = parse_u32(payload, 0)
                _, offset = parse_u32(payload, offset)
                key, offset = parse_u32(payload, offset)
                state, _ = parse_u32(payload, offset)
                pressed = state == 1
                if key in (42, 54):
                    shift_down = pressed
                if pressed:
                    out = translate_key(key, shift_down)
                    if out:
                        os.write(master_fd, out)
            elif interface == "wl_keyboard" and object_id == keyboard_id and opcode == 0:
                # keymap, ignore payload data
                pass
            elif interface == "wl_display" and opcode == 0:
                try:
                    obj_id, offset = parse_u32(payload, 0)
                    code, offset = parse_u32(payload, offset)
                    message, _ = parse_string(payload, offset)
                    log(f"wl_display error obj={obj_id} code={code} message={message}")
                except Exception:
                    log("wl_display error (failed to decode payload)")
                running = False
        if master_fd in readable:
            try:
                data = os.read(master_fd, 4096)
                if not data:
                    running = False
                else:
                    SCREEN.write(data)
                    dirty = True
            except OSError:
                running = False
        if dirty and configured:
            draw_text(buffer_map, cols, rows, (SCREEN.cursor_x, SCREEN.cursor_y), title)
            try:
                conn.send(
                    surface_id,
                    1,
                    pack_u32(shm_buffer.buffer_id) + pack_i32(0) + pack_i32(0),
                )
                conn.send(
                    surface_id,
                    2,
                    pack_i32(0) + pack_i32(0) + pack_i32(width) + pack_i32(height),
                )
                conn.send(surface_id, 6)
            except OSError as err:
                log(f"send error: {err}")
                running = False
            dirty = False
        time.sleep(0.01)

    try:
        os.kill(child_pid, signal.SIGTERM)
    except OSError:
        pass
    buffer_map.close()
    conn.close()
    log("macland-term exit")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as err:  # pragma: no cover
        log(f"macland-term error: {err}")
        raise
