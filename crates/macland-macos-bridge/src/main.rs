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

const FRAME_RATE: u32 = 10;

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
    x: i32,
    y: i32,
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
        }
    }

    fn handle_frame(&mut self, qh: &QueueHandle<BridgeState>) {
        self.frame_count += 1;

        // Only enumerate windows once per second
        if self.last_enum.elapsed().as_secs() >= 1 {
            let start = std::time::Instant::now();
            self.cached_windows = get_macos_windows();
            let enum_time = start.elapsed();
            eprintln!("macland-macos-bridge: enum: {:?}", enum_time);
            self.last_enum = std::time::Instant::now();
        }

        // Skip every other frame for performance
        if self.frame_count % 2 != 0 {
            return;
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

        for (pid, mac_window) in &self.cached_windows {
            if !self.windows.contains_key(pid) {
                let surface = self.compositor.create_surface(qh, ());
                let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface, qh, ());
                let xdg_toplevel = xdg_surface.get_toplevel(qh, ());
                xdg_toplevel.set_title(format!("{} (PID:{})", mac_window.name, pid));
                xdg_toplevel.set_app_id("com.macland.bridge".to_string());
                // Start with minimum size to force compositor to set actual size
                xdg_toplevel.set_min_size(100, 100);
                surface.commit();
                self.windows.insert(
                    *pid,
                    WaylandWindow {
                        surface,
                        xdg_surface,
                        xdg_toplevel,
                        buffer: None,
                        width: 100,
                        height: 100,
                        configured: false,
                    },
                );
            }
        }

        let updates: Vec<(u32, WindowFrame)> = {
            let mut result = Vec::new();
            let mac_windows = &self.cached_windows;

            let capture_start = std::time::Instant::now();
            for (pid, wayland_window) in &mut self.windows {
                if !wayland_window.configured {
                    continue;
                }
                // Use Wayland window size for capture
                if wayland_window.width < 100 || wayland_window.height < 100 {
                    continue;
                }
                // Get macOS window position for capture
                if let Some(mac_window) = mac_windows.get(pid) {
                    if let Some(frame) = capture_window(
                        mac_window.x,
                        mac_window.y,
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
        if frame.pixels.len() != size {
            return Err("size mismatch".to_string());
        }
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
            wl_shm::Format::Bgra8888,
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
fn get_macos_windows() -> HashMap<u32, MacWindow> {
    use std::process::Command;
    let output = Command::new("osascript")
        .args(["-e", "tell application \"System Events\"\nset windowList to {}\nrepeat with p in (every process whose background only is false and name is not \"macland-macos-bridge\")\ntry\nset pidVal to id of p\nset pName to name of p\nrepeat with w in (every window of p)\nset wPos to position of w\nset wSize to size of w\nif (item 1 of wSize) > 50 then\nset end of windowList to {pidVal, pName, item 1 of wPos, item 2 of wPos, item 1 of wSize, item 2 of wSize}\nend if\nend repeat\nend try\nend repeat\nreturn windowList\nend tell"])
        .output();
    let mut windows = HashMap::new();
    if let Ok(out) = output {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.split(", ").collect();
                if parts.len() >= 4 {
                    if let (Ok(pid), Ok(x), Ok(y)) =
                        (parts[0].parse(), parts[2].parse(), parts[3].parse())
                    {
                        windows.insert(
                            pid,
                            MacWindow {
                                _pid: pid,
                                name: parts[1].to_string(),
                                x,
                                y,
                            },
                        );
                    }
                }
            }
        }
    }
    windows
}

#[cfg(target_os = "macos")]
fn capture_window(x: i32, y: i32, width: u32, height: u32) -> Option<WindowFrame> {
    use std::process::Command;

    if width < 50 || height < 50 {
        return None;
    }

    let output = Command::new("screencapture")
        .args([
            "-x",
            "-R",
            &format!("{},{},{},{}", x, y, width, height),
            "/tmp/macland_capture.png",
        ])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let img = image::open("/tmp/macland_capture.png").unwrap();
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if w > 0 && h > 0 {
                return Some(WindowFrame {
                    width: w,
                    height: h,
                    pixels: rgba.into_raw(),
                });
            }
        }
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
                for w in _state.windows.values_mut() {
                    if w.xdg_toplevel == *toplevel && width > 0 && height > 0 {
                        w.width = width as u32;
                        w.height = height as u32;
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
