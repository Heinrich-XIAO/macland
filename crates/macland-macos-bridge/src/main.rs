use std::collections::HashMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};

const FRAME_RATE: u32 = 5;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("macland-macos-bridge: {err}");
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> Result<(), String> {
    eprintln!("macland-macos-bridge: starting...");

    #[cfg(not(target_os = "macos"))]
    return Err("macland-macos-bridge must run on macOS".to_string());

    #[cfg(target_os = "macos")]
    {
        let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
        let (globals, mut event_queue) =
            registry_queue_init(&connection).map_err(|err| err.to_string())?;
        let queue_handle = event_queue.handle();

        let compositor: WlCompositor = globals
            .bind(&queue_handle, 1..=WlCompositor::interface().version, ())
            .map_err(|e| e.to_string())?;
        let xdg_wm_base: XdgWmBase = globals
            .bind(&queue_handle, 1..=XdgWmBase::interface().version, ())
            .map_err(|e| e.to_string())?;
        let shm: WlShm = globals
            .bind(&queue_handle, 1..=WlShm::interface().version, ())
            .map_err(|e| e.to_string())?;

        let mut state = BridgeState::new(compositor, xdg_wm_base, shm);

        let frame_interval = Duration::from_millis(1000 / FRAME_RATE as u64);

        loop {
            let frame_start = std::time::Instant::now();
            event_queue
                .roundtrip(&mut state)
                .map_err(|e| e.to_string())?;
            state.handle_frame(&queue_handle);
            connection.flush().map_err(|e| e.to_string())?;
            let elapsed = frame_start.elapsed();
            if elapsed < frame_interval {
                std::thread::sleep(frame_interval - elapsed);
            }
        }
    }
}

struct MacWindow {
    _pid: u32,
    name: String,
    window_id: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct WaylandWindow {
    surface: WlSurface,
    xdg_surface: XdgSurface,
    xdg_toplevel: XdgToplevel,
    buffer: Option<WlBuffer>,
    width: u32,
    height: u32,
    configured: bool,
}

struct BridgeState {
    compositor: WlCompositor,
    xdg_wm_base: XdgWmBase,
    shm: WlShm,
    windows: HashMap<u32, WaylandWindow>,
    frame_count: u32,
    last_enum: std::time::Instant,
    cached_windows: HashMap<u32, MacWindow>,
    last_window_count: usize,
}

impl BridgeState {
    fn new(compositor: WlCompositor, xdg_wm_base: XdgWmBase, shm: WlShm) -> Self {
        Self {
            compositor,
            xdg_wm_base,
            shm,
            windows: HashMap::new(),
            frame_count: 0,
            last_enum: std::time::Instant::now(),
            cached_windows: HashMap::new(),
            last_window_count: 0,
        }
    }

    fn handle_frame(&mut self, qh: &QueueHandle<BridgeState>) {
        self.frame_count += 1;

        // Only enumerate when window count changes or every 3 seconds
        let current_count = self.windows.len();
        let needs_enum =
            self.last_window_count != current_count || self.last_enum.elapsed().as_secs() >= 3;

        if needs_enum {
            let start = std::time::Instant::now();
            let new_windows = get_macos_windows();
            let enum_time = start.elapsed();
            eprintln!("macland-macos-bridge: enum: {:?}", enum_time);
            self.cached_windows = new_windows;
            self.last_enum = std::time::Instant::now();
            self.last_window_count = self.cached_windows.len();
        }

        let capture_start = std::time::Instant::now();

        for pid in self
            .windows
            .keys()
            .filter(|p| !self.cached_windows.contains_key(p))
            .copied()
            .collect::<Vec<_>>()
        {
            eprintln!("macland-macos-bridge: window {} closed", pid);
            self.windows.remove(&pid);
        }

        // Create windows for found processes
        for (pid, mac_window) in &self.cached_windows {
            if !self.windows.contains_key(pid) {
                eprintln!(
                    "macland-macos-bridge: creating Wayland window for {} (PID:{})",
                    mac_window.name, pid
                );
                let surface = self.compositor.create_surface(qh, ());
                let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface, qh, ());
                let xdg_toplevel = xdg_surface.get_toplevel(qh, ());
                xdg_toplevel.set_title(format!("macOS Bridge - {} (PID:{})", mac_window.name, pid));
                xdg_toplevel.set_app_id("com.macland.bridge".to_string());
                // Set a reasonable default size
                xdg_toplevel.set_min_size(400, 300);
                surface.commit();
                self.windows.insert(
                    *pid,
                    WaylandWindow {
                        surface,
                        xdg_surface,
                        xdg_toplevel,
                        buffer: None,
                        width: 800,
                        height: 600,
                        configured: false,
                    },
                );
                eprintln!(
                    "macland-macos-bridge: created Wayland window for PID:{}",
                    pid
                );
            }
        }

        let updates: Vec<(u32, WindowFrame)> = {
            let mut result = Vec::new();
            let mac_windows = &self.cached_windows;

            let _capture_start = std::time::Instant::now();
            for (pid, wayland_window) in &mut self.windows {
                if !wayland_window.configured {
                    continue;
                }
                // Use Wayland window size for capture
                if wayland_window.width < 100 || wayland_window.height < 100 {
                    continue;
                }
                // Get macOS window for capture
                if let Some(mac_window) = mac_windows.get(pid) {
                    if let Some(frame) = capture_window(
                        mac_window.window_id,
                        wayland_window.width,
                        wayland_window.height,
                    ) {
                        result.push((*pid, frame));
                    }
                }
            }
            let capture_time = capture_start.elapsed();
            eprintln!("macland-macos-bridge: capture: {:?}", capture_time);
            result
        };

        for (pid, frame) in updates {
            let update_start = std::time::Instant::now();
            if let Err(e) = self.update_window(pid, &frame, qh) {
                eprintln!("macland-macos-bridge: update {} failed: {}", pid, e);
            }
            eprintln!(
                "macland-macos-bridge: update time: {:?}",
                update_start.elapsed()
            );
        }
    }

    fn update_window(
        &mut self,
        pid: u32,
        frame: &WindowFrame,
        qh: &QueueHandle<BridgeState>,
    ) -> Result<(), String> {
        let window = self.windows.get_mut(&pid).ok_or("window not found")?;
        let stride = frame.width as i32 * 4;
        let size = (stride as usize) * (frame.height as usize);
        eprintln!(
            "macland-macos-bridge: update: win {}x{} frame {}x{}",
            window.width, window.height, frame.width, frame.height
        );
        if frame.pixels.len() != size {
            return Err("size mismatch".to_string());
        }

        // Debug: print first pixel going to compositor
        eprintln!(
            "macland-macos-bridge: shm first pixel: {:02x?}",
            &frame.pixels[..4]
        );

        let mut backing = create_shm_file(size)?;
        backing
            .write_all(&frame.pixels)
            .map_err(|e| e.to_string())?;
        backing
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let pool = self.shm.create_pool(backing.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            frame.width as i32,
            frame.height as i32,
            stride,
            wl_shm::Format::Abgr8888,
            qh,
            (),
        );
        window.surface.attach(Some(&buffer), 0, 0);
        window.surface.commit();
        window.buffer = Some(buffer);
        Ok(())
    }
}

#[derive(Clone)]
struct WindowFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn resize_macos_window(pid: u32, width: u32, height: u32) {
    use std::process::Command;
    let script = format!(
        "tell application \"System Events\"\nset p to first process whose id is {}\nset size of first window of p to {{{}, {}}}\nend tell",
        pid, width, height
    );
    let _ = Command::new("osascript").args(["-e", &script]).output();
}

#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
fn get_macos_windows() -> HashMap<u32, MacWindow> {
    use std::process::Command;

    // Try to get windows using Python with pyobjc, or fall back to a simple method
    // First, let's try using screencapture to list windows
    let output = Command::new("screencapture").args(["-l", ""]).output();

    // If that doesn't work, use AppleScript which does return real window IDs
    // Actually, AppleScript returns the process ID, not the window ID.
    // Let's try a different approach - use the window title as the identifier
    // and capture using window selection mode

    // For now, let's try to capture using a Python script with pyobjc
    let script = r#"
import subprocess
import sys

try:
    from Quartz import CGWindowListCopyWindowInfo, kCGWindowListOptionOnScreenOnly, kCGNullWindowID
    
    window_list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID)
    windows = []
    for w in window_list:
        pid = w.get('kCGWindowOwnerPID', 0)
        name = w.get('kCGWindowName', '')
        owner_name = w.get('kCGWindowOwnerName', '')
        window_id = w.get('kCGWindowNumber', 0)
        bounds = w.get('kCGWindowBounds', {})
        
        # Skip system windows and our own
        if owner_name and 'macland' not in owner_name.lower() and owner_name not in ['Window Server', 'loginwindow']:
            windows.append({
                'pid': pid,
                'window_id': window_id,
                'name': name or owner_name,
                'x': int(bounds.get('X', 0)),
                'y': int(bounds.get('Y', 0)),
                'width': int(bounds.get('Width', 800)),
                'height': int(bounds.get('Height', 600))
            })
    
    for w in windows:
        print(f"{w['pid']}|{w['window_id']}|{w['name']}|{w['x']}|{w['y']}|{w['width']}|{w['height']}")
except ImportError:
    # pyobjc not available, try using screencapture interactively
    sys.exit(1)
"#;

    let output = Command::new("python3").args(["-c", script]).output();

    let mut windows = HashMap::new();
    if let Ok(out) = output {
        if out.status.success() {
            let output_str = String::from_utf8_lossy(&out.stdout);
            for (i, line) in output_str.lines().enumerate() {
                // Parse: pid|window_id|name|x|y|width|height
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 7 {
                    if let (Ok(pid), Ok(window_id), Ok(x), Ok(y), Ok(width), Ok(height)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u32>(),
                        parts[3].parse::<i32>(),
                        parts[4].parse::<i32>(),
                        parts[5].parse::<u32>(),
                        parts[6].parse::<u32>(),
                    ) {
                        let name = parts[2].to_string();
                        eprintln!("macland-macos-bridge: found window: pid={}, win_id={}, name={}, bounds={}x{}+{}+{}",
                            pid, window_id, name, width, height, x, y);
                        windows.insert(
                            window_id,
                            MacWindow {
                                _pid: pid,
                                name: format!("{} (PID:{})", name, pid),
                                window_id,
                                x,
                                y,
                                width,
                                height,
                            },
                        );
                    }
                }
            }
        }
    }

    // Fallback: if no windows found, try the AppleScript approach
    if windows.is_empty() {
        eprintln!("macland-macos-bridge: Python script failed, falling back to AppleScript");
        let script = r#"tell application "System Events"
set output to ""
repeat with p in (every process whose background only is false)
try
set pid to id of p
set pname to name of p
repeat with w in (every window of p)
try
set sz to size of w
set pos to position of w
if (item 1 of sz) > 30 then
set output to output & pid & "|" & pname & "|" & (item 1 of pos) & "|" & (item 2 of pos) & "|" & (item 1 of sz) & "|" & (item 2 of sz) & "
"
end if
end try
end repeat
end try
end repeat
return output
end tell"#;

        let output = Command::new("osascript").args(["-e", &script]).output();

        if let Ok(out) = output {
            if out.status.success() {
                let output_str = String::from_utf8_lossy(&out.stdout);
                for (i, line) in output_str.lines().enumerate() {
                    // Parse: pid|name|x|y|width|height
                    let parts: Vec<&str> = line.split('|').collect();
                    if parts.len() >= 6 {
                        if let (Ok(pid), Ok(x), Ok(y), Ok(width), Ok(height)) = (
                            parts[0].parse::<u32>(),
                            parts[2].parse::<i32>(),
                            parts[3].parse::<i32>(),
                            parts[4].parse::<u32>(),
                            parts[5].parse::<u32>(),
                        ) {
                            let name = parts[1].to_string();
                            if !name.contains("macland") {
                                // Generate a synthetic window ID for capture
                                // This won't work with screencapture -l, but we can try the rectangle method
                                let window_id = pid * 1000 + i as u32;
                                eprintln!("macland-macos-bridge: found window (synthetic ID): pid={}, win_id={}, name={}, bounds={}x{}+{}+{}",
                                    pid, window_id, name, width, height, x, y);
                                windows.insert(
                                    window_id,
                                    MacWindow {
                                        _pid: pid,
                                        name: format!("{} (PID:{})", name, pid),
                                        window_id,
                                        x,
                                        y,
                                        width,
                                        height,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "macland-macos-bridge: total windows found: {}",
        windows.len()
    );
    windows
}

#[cfg(target_os = "macos")]
fn capture_window(window_id: u32, width: u32, height: u32) -> Option<WindowFrame> {
    use std::process::Command;

    if width < 10 || height < 10 {
        return None;
    }

    let capture_start = std::time::Instant::now();

    // Capture specific window by ID
    let output = Command::new("screencapture")
        .args([
            "-x",
            "-l",
            &window_id.to_string(),
            "/tmp/macland_capture.png",
        ])
        .output();

    let screencapture_time = capture_start.elapsed();

    if let Ok(output) = output {
        if !output.status.success() {
            // Try rectangle capture as fallback
            eprintln!(
                "macland-macos-bridge: window ID capture failed, trying screen capture: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
            let output = Command::new("screencapture")
                .args(["-x", "/tmp/macland_capture.png"])
                .output();
            if let Ok(output) = output {
                if !output.status.success() {
                    eprintln!(
                        "macland-macos-bridge: screen capture failed: {:?}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    return None;
                }
            } else {
                return None;
            }
        }

        let load_start = std::time::Instant::now();
        let img = match image::open("/tmp/macland_capture.png") {
            Ok(img) => img,
            Err(e) => {
                eprintln!(
                    "macland-macos-bridge: image open failed for window {}: {}",
                    window_id, e
                );
                return None;
            }
        };
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();

        let load_time = load_start.elapsed();

        eprintln!(
            "macland-macos-bridge: capture: window_id={}, screencapture={:?}, load={:?}, size={}x{}",
            window_id, screencapture_time, load_time, w, h
        );

        if w > 0 && h > 0 {
            return Some(WindowFrame {
                width: w,
                height: h,
                pixels: rgba.into_raw(),
            });
        }
    } else {
        eprintln!(
            "macland-macos-bridge: screencapture command failed for window {}: {:?}",
            window_id,
            output.err()
        );
    }
    None
}

fn create_shm_file(size: usize) -> Result<File, String> {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let root = PathBuf::from(runtime_dir);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    for attempt in 0..32 {
        let path = root.join(format!(
            "macland-bridge-{}-{}.bin",
            std::process::id(),
            attempt
        ));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                file.set_len(size as u64).map_err(|e| e.to_string())?;
                return Ok(file);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("failed to create shm file".to_string())
}

impl Dispatch<WlCompositor, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _: &WlCompositor,
        _: <WlCompositor as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlRegistry, GlobalListContents> for BridgeState {
    fn event(
        _state: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<XdgWmBase, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        xdg: &XdgWmBase,
        event: <XdgWmBase as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            xdg.pong(serial);
        }
    }
}
impl Dispatch<WlSurface, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _: &WlSurface,
        _: <WlSurface as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<XdgSurface, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        surface: &XdgSurface,
        event: <XdgSurface as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            for w in _state.windows.values_mut() {
                if w.xdg_surface == *surface {
                    w.xdg_surface.ack_configure(serial);
                    w.configured = true;
                }
            }
        }
    }
}
impl Dispatch<XdgToplevel, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        toplevel: &XdgToplevel,
        event: <XdgToplevel as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                for (pid, w) in _state.windows.iter_mut() {
                    if w.xdg_toplevel == *toplevel && width > 0 && height > 0 {
                        let new_width = width as u32;
                        let new_height = height as u32;
                        eprintln!(
                            "macland-macos-bridge: configure: {}x{} (was {}x{})",
                            new_width, new_height, w.width, w.height
                        );
                        // Resize the macOS window to match
                        resize_macos_window(*pid, new_width, new_height);
                        w.width = new_width;
                        w.height = new_height;
                        // Commit to apply the new size
                        w.surface.commit();
                    }
                }
            }
            <XdgToplevel as Proxy>::Event::Close => {
                if let Some(pid) = _state
                    .windows
                    .iter()
                    .find(|(_, w)| w.xdg_toplevel == *toplevel)
                    .map(|(p, _)| *p)
                {
                    _state.windows.remove(&pid);
                }
            }
            _ => {}
        }
    }
}
impl Dispatch<WlShm, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _: &WlShm,
        _: <WlShm as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlShmPool, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _: &WlShmPool,
        _: <WlShmPool as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlBuffer, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _: &WlBuffer,
        _: <WlBuffer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
