#!/usr/bin/env python3

from __future__ import annotations

import signal
import sys
import time

from wayland_capture import (
    WL_DISPLAY_ID,
    WaylandSocket,
    bind,
    create_demo_toplevel,
    discover_globals,
    pack_string,
    parse_u32,
)


running = True


def stop(_signum, _frame) -> None:
    global running
    running = False


def main(argv: list[str]) -> int:
    runtime_dir = None
    display_name = None
    title = "macland demo"
    i = 1
    while i < len(argv):
        if argv[i] == "--title" and i + 1 < len(argv):
            title = argv[i + 1]
            i += 2
            continue
        i += 1

    runtime_dir = __import__("os").environ.get("XDG_RUNTIME_DIR")
    display_name = __import__("os").environ.get("WAYLAND_DISPLAY")
    if not runtime_dir or not display_name:
        print("XDG_RUNTIME_DIR and WAYLAND_DISPLAY are required", file=sys.stderr)
        return 1

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)

    conn = WaylandSocket(runtime_dir, display_name)
    try:
        globals_found = discover_globals(conn)
        registry_id = 2
        shm_id = bind(conn, registry_id, globals_found.shm_name, "wl_shm", 1)
        surface_id, xdg_surface_id, toplevel_id, demo_buffer = create_demo_toplevel(
            conn,
            globals_found,
            shm_id,
            runtime_dir,
        )
        conn.send(toplevel_id, 2, pack_string(title))

        while running:
            try:
                object_id, interface, opcode, payload = conn.recv(timeout=1.0)
            except TimeoutError:
                continue
            except OSError:
                break

            if interface == "xdg_wm_base" and opcode == 0:
                serial, _ = parse_u32(payload, 0)
                conn.send(object_id, 3, __import__("struct").pack("<I", serial))
            elif interface == "xdg_surface" and object_id == xdg_surface_id and opcode == 0:
                serial, _ = parse_u32(payload, 0)
                conn.send(xdg_surface_id, 4, __import__("struct").pack("<I", serial))
            elif interface == "wl_display" and opcode == 0:
                break

        conn.send(toplevel_id, 0)
        conn.send(xdg_surface_id, 0)
        conn.send(surface_id, 0)
        conn.send(demo_buffer.buffer_id, 0)
        conn.send(demo_buffer.pool_id, 1)
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
