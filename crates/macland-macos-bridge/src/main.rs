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
    mac_pid: u32,
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
            eprintln!(
                "macland-macos-bridge: enum result count: cached_before={} new={} wayland_before={}",
                self.cached_windows.len(),
                new_windows.len(),
                self.windows.len()
            );
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
                let initial_width = mac_window.width.max(1);
                let initial_height = mac_window.height.max(1);
                eprintln!(
                    "macland-macos-bridge: creating Wayland window: key={} mac_pid={} mac_window_id={} title={:?} mac_bounds={}x{}+{}+{} initial_wayland={}x{} configured=false",
                    pid,
                    mac_window._pid,
                    mac_window.window_id,
                    mac_window.name,
                    mac_window.width,
                    mac_window.height,
                    mac_window.x,
                    mac_window.y,
                    initial_width,
                    initial_height
                );
                let surface = self.compositor.create_surface(qh, ());
                let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface, qh, ());
                let xdg_toplevel = xdg_surface.get_toplevel(qh, ());
                xdg_toplevel.set_title(format!(
                    "macOS Bridge - {} (PID:{})",
                    mac_window.name, mac_window._pid
                ));
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
                        mac_pid: mac_window._pid,
                        width: initial_width,
                        height: initial_height,
                        configured: false,
                    },
                );
                eprintln!(
                    "macland-macos-bridge: created Wayland window: key={} surface_committed=true min_size=400x300 stored_wayland={}x{} configured=false",
                    pid,
                    initial_width,
                    initial_height
                );
            }
        }

        let updates: Vec<(u32, WindowFrame)> = {
            let mut result = Vec::new();
            let mac_windows = &self.cached_windows;

            let _capture_start = std::time::Instant::now();
            for (pid, wayland_window) in &mut self.windows {
                if !wayland_window.configured {
                    eprintln!(
                        "macland-macos-bridge: skipping capture: key={} reason=not-configured wayland={}x{}",
                        pid, wayland_window.width, wayland_window.height
                    );
                    continue;
                }
                // Use Wayland window size for capture
                if wayland_window.width < 100 || wayland_window.height < 100 {
                    eprintln!(
                        "macland-macos-bridge: skipping capture: key={} reason=too-small wayland={}x{}",
                        pid, wayland_window.width, wayland_window.height
                    );
                    continue;
                }
                // Get macOS window for capture
                if let Some(mac_window) = mac_windows.get(pid) {
                    eprintln!(
                        "macland-macos-bridge: capture request: key={} mac_pid={} mac_window_id={} mac_bounds={}x{}+{}+{} target_wayland={}x{} configured={}",
                        pid,
                        mac_window._pid,
                        mac_window.window_id,
                        mac_window.width,
                        mac_window.height,
                        mac_window.x,
                        mac_window.y,
                        wayland_window.width,
                        wayland_window.height,
                        wayland_window.configured
                    );
                    if let Some(frame) = capture_window(
                        mac_window.window_id,
                        wayland_window.width,
                        wayland_window.height,
                    ) {
                        eprintln!(
                            "macland-macos-bridge: capture success queued: key={} frame={}x{}",
                            pid, frame.width, frame.height
                        );
                        result.push((*pid, frame));
                    } else {
                        eprintln!(
                            "macland-macos-bridge: capture returned none: key={} mac_window_id={}",
                            pid, mac_window.window_id
                        );
                    }
                } else {
                    eprintln!(
                        "macland-macos-bridge: no cached mac window for wayland key={}",
                        pid
                    );
                }
            }
            let capture_time = capture_start.elapsed();
            eprintln!(
                "macland-macos-bridge: capture loop complete: {:?} queued_updates={}",
                capture_time,
                result.len()
            );
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
            "macland-macos-bridge: update begin: key={} wayland={}x{} frame={}x{} stride={} bytes={}",
            pid,
            window.width,
            window.height,
            frame.width,
            frame.height,
            stride,
            size
        );
        if frame.pixels.len() != size {
            eprintln!(
                "macland-macos-bridge: update size mismatch: key={} expected={} actual={}",
                pid,
                size,
                frame.pixels.len()
            );
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
        eprintln!(
            "macland-macos-bridge: update committed: key={} attached_buffer={}x{} stored_wayland={}x{}",
            pid,
            frame.width,
            frame.height,
            window.width,
            window.height
        );
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
fn get_macos_windows() -> HashMap<u32, MacWindow> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
        kCGWindowListOptionOnScreenOnly,
    };

    let mut windows = HashMap::new();
    let option = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;

    if let Some(window_info) = copy_window_info(option, kCGNullWindowID) {
        let key_owner_pid = CFString::new("kCGWindowOwnerPID");
        let key_window_number = CFString::new("kCGWindowNumber");
        let key_window_name = CFString::new("kCGWindowName");
        let key_owner_name = CFString::new("kCGWindowOwnerName");
        let key_bounds = CFString::new("kCGWindowBounds");
        let key_x = CFString::new("X");
        let key_y = CFString::new("Y");
        let key_width = CFString::new("Width");
        let key_height = CFString::new("Height");

        for (index, dict_ref) in window_info.iter().enumerate() {
            let dict_cf_type = unsafe { CFType::wrap_under_get_rule(*dict_ref) };
            let Some(dict_untyped) = dict_cf_type.downcast::<CFDictionary>() else {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} non-dictionary window record",
                    index
                );
                continue;
            };
            let dict: CFDictionary<CFString, CFType> =
                unsafe { CFDictionary::wrap_under_get_rule(dict_untyped.as_concrete_TypeRef()) };

            let pid = dict
                .find(key_owner_pid.clone())
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i64())
                .map(|v| v as u32);
            let window_id = dict
                .find(key_window_number.clone())
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i64())
                .map(|v| v as u32);
            let owner_name = dict
                .find(key_owner_name.clone())
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let window_name = dict
                .find(key_window_name.clone())
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let Some(pid) = pid else {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} missing pid owner={:?} title={:?}",
                    index, owner_name, window_name
                );
                continue;
            };
            let Some(window_id) = window_id else {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} missing window_id pid={} owner={:?} title={:?}",
                    index, pid, owner_name, window_name
                );
                continue;
            };

            if owner_name.contains("macland")
                || owner_name == "Window Server"
                || owner_name == "Dock"
                || owner_name == "loginwindow"
            {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} filtered owner={:?} pid={} window_id={}",
                    index, owner_name, pid, window_id
                );
                continue;
            }

            let Some(bounds_cf) = dict
                .find(key_bounds.clone())
                .and_then(|v| v.downcast::<CFDictionary>())
            else {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} missing bounds pid={} window_id={} owner={:?} title={:?}",
                    index, pid, window_id, owner_name, window_name
                );
                continue;
            };

            let bounds_cf_type = bounds_cf.to_untyped().into_CFType();
            let Some(bounds_untyped) = bounds_cf_type.downcast::<CFDictionary>() else {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} bounds downcast failed pid={} window_id={}",
                    index, pid, window_id
                );
                continue;
            };
            let bounds: CFDictionary<CFString, CFType> =
                unsafe { CFDictionary::wrap_under_get_rule(bounds_untyped.as_concrete_TypeRef()) };

            let x = bounds
                .find(key_x.clone())
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_f64())
                .map(|v| v as i32)
                .unwrap_or(0);
            let y = bounds
                .find(key_y.clone())
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_f64())
                .map(|v| v as i32)
                .unwrap_or(0);
            let width = bounds
                .find(key_width.clone())
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_f64())
                .map(|v| v as u32)
                .unwrap_or(0);
            let height = bounds
                .find(key_height.clone())
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_f64())
                .map(|v| v as u32)
                .unwrap_or(0);

            eprintln!(
                "macland-macos-bridge: quartz raw {} pid={} real_window_id={} owner={:?} title={:?} bounds={}x{}+{}+{}",
                index, pid, window_id, owner_name, window_name, width, height, x, y
            );

            if width <= 30 || height <= 30 {
                eprintln!(
                    "macland-macos-bridge: quartz skip {} too small pid={} window_id={} bounds={}x{}+{}+{}",
                    index, pid, window_id, width, height, x, y
                );
                continue;
            }

            let display_name = if window_name.is_empty() {
                owner_name.clone()
            } else {
                format!("{} - {}", owner_name, window_name)
            };

            windows.insert(
                window_id,
                MacWindow {
                    _pid: pid,
                    name: display_name,
                    window_id,
                    x,
                    y,
                    width,
                    height,
                },
            );
        }
    } else {
        eprintln!("macland-macos-bridge: quartz copy_window_info returned none");
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
        eprintln!(
            "macland-macos-bridge: capture abort: window_id={} requested={}x{} too small",
            window_id, width, height
        );
        return None;
    }

    let capture_start = std::time::Instant::now();

    // Capture specific window by ID
    let output = Command::new("screencapture")
        .args([
            "-x",
            "-o",
            "-l",
            &window_id.to_string(),
            "/tmp/macland_capture.png",
        ])
        .output();

    let screencapture_time = capture_start.elapsed();

    if let Ok(output) = output {
        if !output.status.success() {
            eprintln!(
                "macland-macos-bridge: capture failed for window {}: {:?}",
                window_id,
                String::from_utf8_lossy(&output.stderr)
            );
            return None;
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
                    eprintln!(
                        "macland-macos-bridge: xdg_surface configure: serial={} old_configured={} current_wayland={}x{}",
                        serial,
                        w.configured,
                        w.width,
                        w.height
                    );
                    w.xdg_surface.ack_configure(serial);
                    w.configured = true;
                    eprintln!(
                        "macland-macos-bridge: xdg_surface configured: serial={} new_configured={} current_wayland={}x{}",
                        serial,
                        w.configured,
                        w.width,
                        w.height
                    );
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
                    if w.xdg_toplevel == *toplevel {
                        let resolved_width = if width > 0 { width as u32 } else { w.width };
                        let resolved_height = if height > 0 { height as u32 } else { w.height };
                        eprintln!(
                            "macland-macos-bridge: xdg_toplevel configure: key={} raw={}x{} resolved={}x{} old={}x{} configured={}",
                            pid,
                            width,
                            height,
                            resolved_width,
                            resolved_height,
                            w.width,
                            w.height,
                            w.configured
                        );
                        if width <= 0 || height <= 0 {
                            eprintln!(
                                "macland-macos-bridge: xdg_toplevel configure preserved previous size: key={} raw={}x{} stored_wayland={}x{}",
                                pid,
                                width,
                                height,
                                w.width,
                                w.height
                            );
                            continue;
                        }
                        // Resize the macOS window to match using the owning process id.
                        resize_macos_window(w.mac_pid, resolved_width, resolved_height);
                        w.width = resolved_width;
                        w.height = resolved_height;
                        // Commit to apply the new size
                        w.surface.commit();
                        eprintln!(
                            "macland-macos-bridge: xdg_toplevel configure applied: key={} mac_pid={} stored_wayland={}x{} surface_commit=true",
                            pid,
                            w.mac_pid,
                            w.width,
                            w.height
                        );
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
                    eprintln!(
                        "macland-macos-bridge: xdg_toplevel close: key={} removing_window=true",
                        pid
                    );
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
