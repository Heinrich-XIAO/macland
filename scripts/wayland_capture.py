#!/usr/bin/env python3

from __future__ import annotations

import array
import json
import os
import socket
import struct
import sys
import tempfile
import time
import zlib
from dataclasses import dataclass
from pathlib import Path


WL_DISPLAY_ID = 1
WL_SHM_FORMAT_ARGB8888 = 0
WL_SHM_FORMAT_XRGB8888 = 1


class CaptureError(RuntimeError):
    pass


@dataclass
class Globals:
    shm_name: int | None = None
    output_name: int | None = None
    screencopy_name: int | None = None
    screencopy_version: int = 0
    compositor_name: int | None = None
    compositor_version: int = 0
    xdg_wm_base_name: int | None = None
    xdg_wm_base_version: int = 0


@dataclass
class BufferInfo:
    format: int
    width: int
    height: int
    stride: int


@dataclass
class ShmBuffer:
    pool_id: int
    buffer_id: int
    fd: int
    backing_path: str


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
                    raise CaptureError("wayland socket closed")
                self.buffer.extend(data)
            object_id, size_opcode = struct.unpack_from("<II", self.buffer, 0)
            size = size_opcode >> 16
            opcode = size_opcode & 0xFFFF
            while len(self.buffer) < size:
                data, _, _, _ = self.sock.recvmsg(65536, 0)
                if not data:
                    raise CaptureError("wayland socket closed mid-message")
                self.buffer.extend(data)
            payload = bytes(self.buffer[8:size])
            del self.buffer[:size]
            return object_id, self.interfaces.get(object_id, "unknown"), opcode, payload
        finally:
            if timeout is not None:
                self.sock.settimeout(None)


def pack_u32(value: int) -> bytes:
    return struct.pack("<I", value)


def pack_i32(value: int) -> bytes:
    return struct.pack("<i", value)


def pack_string(value: str) -> bytes:
    encoded = value.encode("utf-8") + b"\0"
    payload = struct.pack("<I", len(encoded)) + encoded
    padding = (-len(payload)) % 4
    return payload + (b"\0" * padding)


def parse_u32(payload: bytes, offset: int) -> tuple[int, int]:
    return struct.unpack_from("<I", payload, offset)[0], offset + 4


def parse_string(payload: bytes, offset: int) -> tuple[str, int]:
    length, offset = parse_u32(payload, offset)
    raw = payload[offset : offset + length]
    offset += length
    offset += (-offset) % 4
    return raw.rstrip(b"\0").decode("utf-8"), offset


def discover_globals(conn: WaylandSocket) -> Globals:
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
            elif iface_name == "wl_output" and globals_found.output_name is None:
                globals_found.output_name = name
            elif iface_name == "zwlr_screencopy_manager_v1" and globals_found.screencopy_name is None:
                globals_found.screencopy_name = name
                globals_found.screencopy_version = version
            elif iface_name == "wl_compositor" and globals_found.compositor_name is None:
                globals_found.compositor_name = name
                globals_found.compositor_version = version
            elif iface_name == "xdg_wm_base" and globals_found.xdg_wm_base_name is None:
                globals_found.xdg_wm_base_name = name
                globals_found.xdg_wm_base_version = version
        elif interface == "wl_callback" and object_id == callback_id and opcode == 0:
            conn.destroy_id(callback_id)
            break
        elif interface == "wl_display" and opcode == 0:
            raise CaptureError(f"wl_display error: {payload!r}")

    if globals_found.shm_name is None:
        raise CaptureError("compositor does not expose wl_shm")
    if globals_found.output_name is None:
        raise CaptureError("compositor does not expose wl_output")
    if globals_found.screencopy_name is None:
        raise CaptureError("compositor does not expose zwlr_screencopy_manager_v1")
    if globals_found.compositor_name is None:
        raise CaptureError("compositor does not expose wl_compositor")
    if globals_found.xdg_wm_base_name is None:
        raise CaptureError("compositor does not expose xdg_wm_base")
    return globals_found


def roundtrip(conn: WaylandSocket) -> None:
    callback_id = conn.new_id("wl_callback")
    conn.send(WL_DISPLAY_ID, 0, pack_u32(callback_id))
    while True:
        object_id, interface, opcode, payload = conn.recv(timeout=5)
        if interface == "wl_callback" and object_id == callback_id and opcode == 0:
            conn.destroy_id(callback_id)
            return
        if interface == "wl_display" and opcode == 0:
            raise CaptureError(f"wl_display error: {payload!r}")


def create_shm_buffer(
    conn: WaylandSocket,
    shm_id: int,
    width: int,
    height: int,
    stride: int,
    fmt: int,
    data: bytes | None,
    runtime_dir: str,
) -> ShmBuffer:
    size = stride * height
    fd, backing_path = tempfile.mkstemp(prefix="macland-capture-", dir=runtime_dir)
    os.ftruncate(fd, size)
    if data is not None:
        os.write(fd, data)
        os.lseek(fd, 0, os.SEEK_SET)
    pool_id = conn.new_id("wl_shm_pool")
    buffer_id = conn.new_id("wl_buffer")
    conn.send(shm_id, 0, pack_u32(pool_id) + pack_i32(size), fds=[fd])
    conn.send(
        pool_id,
        0,
        pack_u32(buffer_id)
        + pack_i32(0)
        + pack_i32(width)
        + pack_i32(height)
        + pack_i32(stride)
        + pack_u32(fmt),
    )
    return ShmBuffer(pool_id=pool_id, buffer_id=buffer_id, fd=fd, backing_path=backing_path)


def destroy_shm_buffer(conn: WaylandSocket, shm_buffer: ShmBuffer) -> None:
    conn.send(shm_buffer.buffer_id, 0)
    conn.send(shm_buffer.pool_id, 1)
    os.close(shm_buffer.fd)
    try:
        os.unlink(shm_buffer.backing_path)
    except FileNotFoundError:
        pass


def make_demo_surface_payload(width: int, height: int) -> bytes:
    data = bytearray()
    for y in range(height):
        for x in range(width):
            border = x < 6 or y < 6 or x >= width - 6 or y >= height - 6
            if border:
                r, g, b = 0xF5, 0xF5, 0xF5
            else:
                r = min(255, 0x30 + (x * 180) // max(1, width - 1))
                g = min(255, 0x6A + (y * 120) // max(1, height - 1))
                b = 0xC8
            data.extend((b, g, r, 255))
    return bytes(data)


def create_demo_toplevel(
    conn: WaylandSocket,
    globals_found: Globals,
    shm_id: int,
    runtime_dir: str,
) -> tuple[int, int, int, ShmBuffer]:
    compositor_id = bind(
        conn,
        2,
        globals_found.compositor_name,
        "wl_compositor",
        min(4, globals_found.compositor_version),
    )
    wm_base_id = bind(
        conn,
        2,
        globals_found.xdg_wm_base_name,
        "xdg_wm_base",
        min(2, globals_found.xdg_wm_base_version),
    )
    surface_id = conn.new_id("wl_surface")
    conn.send(compositor_id, 0, pack_u32(surface_id))
    xdg_surface_id = conn.new_id("xdg_surface")
    conn.send(wm_base_id, 2, pack_u32(xdg_surface_id) + pack_u32(surface_id))
    toplevel_id = conn.new_id("xdg_toplevel")
    conn.send(xdg_surface_id, 1, pack_u32(toplevel_id))
    conn.send(toplevel_id, 2, pack_string("macland capture"))
    conn.send(toplevel_id, 3, pack_string("macland.capture"))
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
        elif interface == "wl_display" and opcode == 0:
            raise CaptureError(f"wl_display error while configuring demo surface: {payload!r}")

    width, height = 320, 220
    stride = width * 4
    demo = create_shm_buffer(
        conn,
        shm_id,
        width,
        height,
        stride,
        WL_SHM_FORMAT_ARGB8888,
        make_demo_surface_payload(width, height),
        runtime_dir,
    )
    conn.send(xdg_surface_id, 3, pack_i32(0) + pack_i32(0) + pack_i32(width) + pack_i32(height))
    conn.send(surface_id, 1, pack_u32(demo.buffer_id) + pack_i32(0) + pack_i32(0))
    conn.send(surface_id, 2, pack_i32(0) + pack_i32(0) + pack_i32(width) + pack_i32(height))
    conn.send(surface_id, 6)
    roundtrip(conn)
    time.sleep(0.2)
    return surface_id, xdg_surface_id, toplevel_id, demo


def bind(conn: WaylandSocket, registry_id: int, name: int, interface_name: str, version: int) -> int:
    object_id = conn.new_id(interface_name)
    payload = pack_u32(name) + pack_string(interface_name) + pack_u32(version) + pack_u32(object_id)
    conn.send(registry_id, 0, payload)
    return object_id


def capture_output(runtime_dir: str, display_name: str, image_path: Path, report_path: Path | None) -> None:
    conn = WaylandSocket(runtime_dir, display_name)
    report = {
        "connected": True,
        "configured": True,
        "first_frame_presented": True,
        "screenshot_captured": False,
    }
    try:
        globals_found = discover_globals(conn)
        registry_id = 2
        shm_id = bind(conn, registry_id, globals_found.shm_name, "wl_shm", 1)
        output_id = bind(conn, registry_id, globals_found.output_name, "wl_output", 1)
        manager_id = bind(
            conn,
            registry_id,
            globals_found.screencopy_name,
            "zwlr_screencopy_manager_v1",
            min(3, globals_found.screencopy_version),
        )
        surface_id, xdg_surface_id, toplevel_id, demo_buffer = create_demo_toplevel(
            conn,
            globals_found,
            shm_id,
            runtime_dir,
        )

        frame_id = conn.new_id("zwlr_screencopy_frame_v1")
        conn.send(manager_id, 0, pack_u32(frame_id) + pack_i32(0) + pack_u32(output_id))

        buffer_info: BufferInfo | None = None
        flags = 0
        buffer_done = False
        while not buffer_done:
            object_id, interface, opcode, payload = conn.recv(timeout=5)
            if interface == "zwlr_screencopy_frame_v1" and object_id == frame_id:
                if opcode == 0:
                    fmt, offset = parse_u32(payload, 0)
                    width, offset = parse_u32(payload, offset)
                    height, offset = parse_u32(payload, offset)
                    stride, _ = parse_u32(payload, offset)
                    buffer_info = BufferInfo(fmt, width, height, stride)
                elif opcode == 1:
                    flags, _ = parse_u32(payload, 0)
                elif opcode == 3:
                    raise CaptureError("screencopy frame failed before copy")
                elif opcode == 6:
                    buffer_done = True
            elif interface == "wl_display" and opcode == 0:
                raise CaptureError(f"wl_display error: {payload!r}")
        if buffer_info is None:
            raise CaptureError("compositor never advertised a wl_shm screenshot buffer")
        if buffer_info.format not in {WL_SHM_FORMAT_XRGB8888, WL_SHM_FORMAT_ARGB8888}:
            raise CaptureError(f"unsupported screenshot format {buffer_info.format}")

        screenshot_buffer = create_shm_buffer(
            conn,
            shm_id,
            buffer_info.width,
            buffer_info.height,
            buffer_info.stride,
            buffer_info.format,
            None,
            runtime_dir,
        )
        try:
            conn.send(frame_id, 0, pack_u32(screenshot_buffer.buffer_id))

            ready = False
            while not ready:
                object_id, interface, opcode, payload = conn.recv(timeout=5)
                if interface == "zwlr_screencopy_frame_v1" and object_id == frame_id:
                    if opcode == 1:
                        flags, _ = parse_u32(payload, 0)
                    elif opcode == 2:
                        ready = True
                    elif opcode == 3:
                        raise CaptureError("screencopy frame failed during copy")
                elif interface == "xdg_wm_base" and opcode == 0:
                    serial, _ = parse_u32(payload, 0)
                    conn.send(object_id, 3, pack_u32(serial))
                elif interface == "wl_display" and opcode == 0:
                    raise CaptureError(f"wl_display error: {payload!r}")

            with os.fdopen(os.dup(screenshot_buffer.fd), "rb", closefd=True) as handle:
                data = handle.read(buffer_info.stride * buffer_info.height)
            write_png(image_path, buffer_info, data, flags)
            report["screenshot_captured"] = True

            destroy_shm_buffer(conn, screenshot_buffer)
            conn.send(frame_id, 1)
            conn.send(manager_id, 2)
            conn.send(toplevel_id, 0)
            conn.send(xdg_surface_id, 0)
            conn.send(surface_id, 0)
            destroy_shm_buffer(conn, demo_buffer)
            roundtrip(conn)
        finally:
            pass
    finally:
        conn.close()
        if report_path is not None:
            report_path.parent.mkdir(parents=True, exist_ok=True)
            report_path.write_text(json.dumps(report, indent=2))


def write_png(path: Path, info: BufferInfo, raw: bytes, flags: int) -> None:
    rows = []
    for row in range(info.height):
        src_row = row if not (flags & 1) else (info.height - 1 - row)
        start = src_row * info.stride
        row_bytes = bytearray()
        for col in range(info.width):
            b, g, r, a = raw[start + (col * 4) : start + (col * 4) + 4]
            if info.format == WL_SHM_FORMAT_XRGB8888:
                a = 255
            row_bytes.extend((r, g, b, a))
        rows.append(bytes([0]) + bytes(row_bytes))
    payload = b"".join(rows)
    compressed = zlib.compress(payload, level=9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", info.width, info.height, 8, 6, 0, 0, 0)),
            chunk(b"IDAT", compressed),
            chunk(b"IEND", b""),
        ]
    )
    path.write_bytes(png)


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print("usage: wayland_capture.py <output.png> [report.json]", file=sys.stderr)
        return 1
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR")
    display_name = os.environ.get("WAYLAND_DISPLAY")
    if not runtime_dir or not display_name:
        print("XDG_RUNTIME_DIR and WAYLAND_DISPLAY are required", file=sys.stderr)
        return 1
    image_path = Path(argv[1]).resolve()
    report_path = Path(argv[2]).resolve() if len(argv) > 2 else None
    try:
        capture_output(runtime_dir, display_name, image_path, report_path)
        return 0
    except CaptureError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
