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
    title: String,
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
    buffers: Option<WindowBuffers>,
    mac_pid: u32,
    width: u32,
    height: u32,
    configured: bool,
}

struct WindowBuffers {
    backing: File,
    width: u32,
    height: u32,
    slots: [WindowBufferSlot; 2],
    next_slot: usize,
}

struct WindowBufferSlot {
    buffer: WlBuffer,
    offset: usize,
    busy: bool,
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
                        buffers: None,
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

        let surface = window.surface.clone();
        let Some(slot_buffer) = ({
            let buffers = ensure_window_buffers(window, &self.shm, frame.width, frame.height, qh)?;
            let Some(slot_index) = next_available_slot(buffers) else {
                eprintln!(
                    "macland-macos-bridge: update skipped: key={} reason=no-free-buffer stored_wayland={}x{}",
                    pid, window.width, window.height
                );
                return Ok(());
            };
            let slot_offset = buffers.slots[slot_index].offset;
            let slot_buffer = buffers.slots[slot_index].buffer.clone();
            buffers
                .backing
                .seek(SeekFrom::Start(slot_offset as u64))
                .map_err(|e: std::io::Error| e.to_string())?;
            buffers
                .backing
                .write_all(&frame.pixels)
                .map_err(|e: std::io::Error| e.to_string())?;
            buffers.slots[slot_index].busy = true;
            Some(slot_buffer)
        }) else {
            return Ok(());
        };
        surface.attach(Some(&slot_buffer), 0, 0);
        surface.commit();
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

fn ensure_window_buffers<'a>(
    window: &'a mut WaylandWindow,
    shm: &WlShm,
    width: u32,
    height: u32,
    qh: &QueueHandle<BridgeState>,
) -> Result<&'a mut WindowBuffers, String> {
    let recreate = match &window.buffers {
        Some(buffers) => buffers.width != width || buffers.height != height,
        None => true,
    };
    if recreate {
        let stride = width as i32 * 4;
        let slot_len = (stride as usize) * (height as usize);
        let total_size = slot_len * 2;
        let backing = create_shm_file(total_size)?;
        let pool = shm.create_pool(backing.as_fd(), total_size as i32, qh, ());
        let first = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride,
            wl_shm::Format::Abgr8888,
            qh,
            (),
        );
        let second = pool.create_buffer(
            slot_len as i32,
            width as i32,
            height as i32,
            stride,
            wl_shm::Format::Abgr8888,
            qh,
            (),
        );
        window.buffers = Some(WindowBuffers {
            backing,
            width,
            height,
            slots: [
                WindowBufferSlot {
                    buffer: first,
                    offset: 0,
                    busy: false,
                },
                WindowBufferSlot {
                    buffer: second,
                    offset: slot_len,
                    busy: false,
                },
            ],
            next_slot: 0,
        });
    }
    window
        .buffers
        .as_mut()
        .ok_or_else(|| "window buffers unavailable".to_string())
}

fn next_available_slot(buffers: &mut WindowBuffers) -> Option<usize> {
    for offset in 0..buffers.slots.len() {
        let index = (buffers.next_slot + offset) % buffers.slots.len();
        if !buffers.slots[index].busy {
            buffers.next_slot = (index + 1) % buffers.slots.len();
            return Some(index);
        }
    }
    None
}

#[derive(Clone)]
struct WindowFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn resize_macos_window(mac_window: &MacWindow, width: u32, height: u32) {
    if ax_resize::resize_window(mac_window, width, height).is_err() {
        eprintln!(
            "macland-macos-bridge: accessibility resize failed: window_id={} pid={} target={}x{} title={:?}",
            mac_window.window_id, mac_window._pid, width, height, mac_window.title
        );
    }
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

            let candidate = MacWindow {
                _pid: pid,
                name: display_name.clone(),
                title: window_name.clone(),
                window_id,
                x,
                y,
                width,
                height,
            };

            match ax_resize::window_is_resizable(&candidate) {
                Ok(true) => {}
                Ok(false) => {
                    eprintln!(
                        "macland-macos-bridge: quartz skip {} non-resizable pid={} window_id={} owner={:?} title={:?}",
                        index, pid, window_id, owner_name, window_name
                    );
                    continue;
                }
                Err(()) => {
                    eprintln!(
                        "macland-macos-bridge: quartz resizable-check failed {} pid={} window_id={} owner={:?} title={:?}; keeping window",
                        index, pid, window_id, owner_name, window_name
                    );
                }
            }

            windows.insert(window_id, candidate);
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
    use quartz_capture::{
        cgrect_null, CGColorSpaceHandle, CGContextHandle, CGImageHandle, CGPoint, CGRect, CGSize,
        K_CG_BITMAP_BYTE_ORDER32_BIG, K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST,
        K_CG_INTERPOLATION_QUALITY_HIGH, K_CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING,
        K_CG_WINDOW_IMAGE_NOMINAL_RESOLUTION, K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW,
    };

    if width < 10 || height < 10 {
        eprintln!(
            "macland-macos-bridge: capture abort: window_id={} requested={}x{} too small",
            window_id, width, height
        );
        return None;
    }

    let capture_start = std::time::Instant::now();

    let image = CGImageHandle::window_image(
        cgrect_null(),
        K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW,
        window_id,
        K_CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING | K_CG_WINDOW_IMAGE_NOMINAL_RESOLUTION,
    );
    let capture_time = capture_start.elapsed();

    if let Some(image) = image {
        let render_start = std::time::Instant::now();
        let stride = width as usize * 4;
        let mut pixels = vec![0; stride * height as usize];
        let color_space = CGColorSpaceHandle::device_rgb()?;
        let bitmap_info = K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST | K_CG_BITMAP_BYTE_ORDER32_BIG;
        let context = CGContextHandle::bitmap_context(
            pixels.as_mut_ptr().cast(),
            width as usize,
            height as usize,
            8,
            stride,
            &color_space,
            bitmap_info,
        )?;
        let source_width = image.width() as f64;
        let source_height = image.height() as f64;
        let target_width = width as f64;
        let target_height = height as f64;
        let scale = f64::min(target_width / source_width, target_height / source_height);
        let fitted_width = (source_width * scale).max(1.0);
        let fitted_height = (source_height * scale).max(1.0);
        let offset_x = ((target_width - fitted_width) / 2.0).max(0.0);
        let offset_y = ((target_height - fitted_height) / 2.0).max(0.0);
        let rect = CGRect::new(
            &CGPoint::new(offset_x, offset_y),
            &CGSize::new(fitted_width, fitted_height),
        );
        context.translate(0.0, height as f64);
        context.scale(1.0, -1.0);
        context.set_interpolation_quality(K_CG_INTERPOLATION_QUALITY_HIGH);
        context.draw_image(rect, &image);
        context.flush();
        let render_time = render_start.elapsed();

        eprintln!(
            "macland-macos-bridge: capture: window_id={}, quartz={:?}, render={:?}, size={}x{} source={}x{} fitted={}x{} offset={}x{}",
            window_id,
            capture_time,
            render_time,
            width,
            height,
            image.width(),
            image.height(),
            fitted_width as u32,
            fitted_height as u32,
            offset_x as u32,
            offset_y as u32
        );

        return Some(WindowFrame {
            width,
            height,
            pixels,
        });
    } else {
        eprintln!(
            "macland-macos-bridge: quartz capture failed for window {} after {:?}",
            window_id, capture_time
        );
    }
    None
}

#[cfg(target_os = "macos")]
mod quartz_capture {
    use std::ffi::c_void;

    pub const K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW: u32 = 1 << 3;
    pub const K_CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING: u32 = 1 << 0;
    pub const K_CG_WINDOW_IMAGE_NOMINAL_RESOLUTION: u32 = 1 << 4;
    pub const K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST: u32 = 1;
    pub const K_CG_BITMAP_BYTE_ORDER32_BIG: u32 = 4 << 12;
    pub const K_CG_INTERPOLATION_QUALITY_HIGH: i32 = 4;

    pub type CGFloat = f64;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGPoint {
        pub x: CGFloat,
        pub y: CGFloat,
    }

    impl CGPoint {
        pub fn new(x: CGFloat, y: CGFloat) -> Self {
            Self { x, y }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGSize {
        pub width: CGFloat,
        pub height: CGFloat,
    }

    impl CGSize {
        pub fn new(width: CGFloat, height: CGFloat) -> Self {
            Self { width, height }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGRect {
        pub origin: CGPoint,
        pub size: CGSize,
    }

    impl CGRect {
        pub fn new(origin: &CGPoint, size: &CGSize) -> Self {
            Self {
                origin: *origin,
                size: *size,
            }
        }
    }

    pub struct CGImageHandle(*mut c_void);

    impl CGImageHandle {
        pub fn window_image(
            screen_bounds: CGRect,
            list_option: u32,
            window_id: u32,
            image_option: u32,
        ) -> Option<Self> {
            let image = unsafe {
                CGWindowListCreateImage(screen_bounds, list_option, window_id, image_option)
            };
            if image.is_null() {
                None
            } else {
                Some(Self(image))
            }
        }

        pub fn width(&self) -> usize {
            unsafe { CGImageGetWidth(self.0) }
        }

        pub fn height(&self) -> usize {
            unsafe { CGImageGetHeight(self.0) }
        }
    }

    impl Drop for CGImageHandle {
        fn drop(&mut self) {
            unsafe { CGImageRelease(self.0) }
        }
    }

    pub struct CGColorSpaceHandle(*mut c_void);

    impl CGColorSpaceHandle {
        pub fn device_rgb() -> Option<Self> {
            let color_space = unsafe { CGColorSpaceCreateDeviceRGB() };
            if color_space.is_null() {
                None
            } else {
                Some(Self(color_space))
            }
        }
    }

    impl Drop for CGColorSpaceHandle {
        fn drop(&mut self) {
            unsafe { CGColorSpaceRelease(self.0) }
        }
    }

    pub struct CGContextHandle(*mut c_void);

    impl CGContextHandle {
        pub fn bitmap_context(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            color_space: &CGColorSpaceHandle,
            bitmap_info: u32,
        ) -> Option<Self> {
            let context = unsafe {
                CGBitmapContextCreate(
                    data,
                    width,
                    height,
                    bits_per_component,
                    bytes_per_row,
                    color_space.0,
                    bitmap_info,
                )
            };
            if context.is_null() {
                None
            } else {
                Some(Self(context))
            }
        }

        pub fn translate(&self, tx: CGFloat, ty: CGFloat) {
            unsafe { CGContextTranslateCTM(self.0, tx, ty) }
        }

        pub fn scale(&self, sx: CGFloat, sy: CGFloat) {
            unsafe { CGContextScaleCTM(self.0, sx, sy) }
        }

        pub fn set_interpolation_quality(&self, quality: i32) {
            unsafe { CGContextSetInterpolationQuality(self.0, quality) }
        }

        pub fn draw_image(&self, rect: CGRect, image: &CGImageHandle) {
            unsafe { CGContextDrawImage(self.0, rect, image.0) }
        }

        pub fn flush(&self) {
            unsafe { CGContextFlush(self.0) }
        }
    }

    impl Drop for CGContextHandle {
        fn drop(&mut self) {
            unsafe { CGContextRelease(self.0) }
        }
    }

    pub fn cgrect_null() -> CGRect {
        unsafe { CGRectNull }
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        static CGRectNull: CGRect;

        fn CGWindowListCreateImage(
            screen_bounds: CGRect,
            list_option: u32,
            window_id: u32,
            image_option: u32,
        ) -> *mut c_void;
        fn CGImageRelease(image: *mut c_void);
        fn CGImageGetWidth(image: *mut c_void) -> usize;
        fn CGImageGetHeight(image: *mut c_void) -> usize;
        fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
        fn CGColorSpaceRelease(color_space: *mut c_void);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            color_space: *mut c_void,
            bitmap_info: u32,
        ) -> *mut c_void;
        fn CGContextRelease(context: *mut c_void);
        fn CGContextTranslateCTM(context: *mut c_void, tx: CGFloat, ty: CGFloat);
        fn CGContextScaleCTM(context: *mut c_void, sx: CGFloat, sy: CGFloat);
        fn CGContextSetInterpolationQuality(context: *mut c_void, quality: i32);
        fn CGContextDrawImage(context: *mut c_void, rect: CGRect, image: *mut c_void);
        fn CGContextFlush(context: *mut c_void);
    }
}

#[cfg(target_os = "macos")]
mod ax_resize {
    use super::MacWindow;
    use std::ffi::{c_char, c_void};
    use std::ptr;

    type CFIndex = isize;
    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFArrayRef = *const c_void;
    type AXUIElementRef = *const c_void;
    type AXValueRef = *const c_void;
    type AXError = i32;
    type AXValueType = u32;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_AX_VALUE_CGPOINT_TYPE: AXValueType = 1;
    const K_AX_VALUE_CGSIZE_TYPE: AXValueType = 2;
    const AX_SUCCESS: AXError = 0;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    struct OwnedCFType(CFTypeRef);

    impl OwnedCFType {
        fn new(ptr: CFTypeRef) -> Option<Self> {
            if ptr.is_null() {
                None
            } else {
                Some(Self(ptr))
            }
        }

        fn as_ptr(&self) -> CFTypeRef {
            self.0
        }
    }

    impl Drop for OwnedCFType {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) }
        }
    }

    pub fn window_is_resizable(mac_window: &MacWindow) -> Result<bool, ()> {
        let app = unsafe { AXUIElementCreateApplication(mac_window._pid as i32) };
        let Some(app) = OwnedCFType::new(app.cast()) else {
            return Err(());
        };
        let windows_attr = cf_string("AXWindows");
        let windows = copy_attribute_value(app.as_ptr().cast(), windows_attr.as_ptr())?;
        let count = unsafe { CFArrayGetCount(windows.as_ptr().cast()) };
        let mut best_window: Option<AXUIElementRef> = None;
        let mut best_score = i64::MIN;

        for index in 0..count {
            let element = unsafe { CFArrayGetValueAtIndex(windows.as_ptr().cast(), index) };
            if element.is_null() {
                continue;
            }
            let title = copy_string_attribute(element.cast(), "AXTitle").unwrap_or_default();
            let position = copy_point_attribute(element.cast(), "AXPosition").unwrap_or_default();
            let size = copy_size_attribute(element.cast(), "AXSize").unwrap_or_default();
            let score = score_window(mac_window, &title, position, size);
            if score > best_score {
                best_score = score;
                best_window = Some(element.cast());
            }
        }

        let Some(window) = best_window else {
            return Err(());
        };
        let size_attr = cf_string("AXSize");
        let mut settable: u8 = 0;
        let result =
            unsafe { AXUIElementIsAttributeSettable(window, size_attr.as_ptr(), &mut settable) };
        if result != AX_SUCCESS {
            return Err(());
        }
        Ok(settable != 0)
    }

    pub fn resize_window(mac_window: &MacWindow, width: u32, height: u32) -> Result<(), ()> {
        let app = unsafe { AXUIElementCreateApplication(mac_window._pid as i32) };
        let Some(app) = OwnedCFType::new(app.cast()) else {
            return Err(());
        };
        let windows_attr = cf_string("AXWindows");
        let windows = copy_attribute_value(app.as_ptr().cast(), windows_attr.as_ptr())?;
        let count = unsafe { CFArrayGetCount(windows.as_ptr().cast()) };
        let mut best_window: Option<AXUIElementRef> = None;
        let mut best_score = i64::MIN;

        for index in 0..count {
            let element = unsafe { CFArrayGetValueAtIndex(windows.as_ptr().cast(), index) };
            if element.is_null() {
                continue;
            }
            let title = copy_string_attribute(element.cast(), "AXTitle").unwrap_or_default();
            let position = copy_point_attribute(element.cast(), "AXPosition").unwrap_or_default();
            let size = copy_size_attribute(element.cast(), "AXSize").unwrap_or_default();
            let score = score_window(mac_window, &title, position, size);
            if score > best_score {
                best_score = score;
                best_window = Some(element.cast());
            }
        }

        let Some(window) = best_window else {
            return Err(());
        };
        let size_attr = cf_string("AXSize");
        let target_size = CGSize {
            width: width as f64,
            height: height as f64,
        };
        let size_value = unsafe {
            AXValueCreate(
                K_AX_VALUE_CGSIZE_TYPE,
                (&target_size as *const CGSize).cast(),
            )
        };
        let Some(size_value) = OwnedCFType::new(size_value.cast()) else {
            return Err(());
        };
        let result = unsafe {
            AXUIElementSetAttributeValue(window, size_attr.as_ptr(), size_value.as_ptr())
        };
        if result == AX_SUCCESS {
            Ok(())
        } else {
            Err(())
        }
    }

    fn score_window(mac_window: &MacWindow, title: &str, position: CGPoint, size: CGSize) -> i64 {
        let mut score = 0i64;
        if title == mac_window.title {
            score += 10_000;
        } else if title.is_empty() && mac_window.title.is_empty() {
            score += 2_000;
        }
        score -= (position.x as i64 - mac_window.x as i64).abs();
        score -= (position.y as i64 - mac_window.y as i64).abs();
        score -= (size.width as i64 - mac_window.width as i64).abs() * 2;
        score -= (size.height as i64 - mac_window.height as i64).abs() * 2;
        score
    }

    fn copy_attribute_value(
        element: AXUIElementRef,
        attribute: CFStringRef,
    ) -> Result<OwnedCFType, ()> {
        let mut value: CFTypeRef = ptr::null();
        let result = unsafe { AXUIElementCopyAttributeValue(element, attribute, &mut value) };
        if result != AX_SUCCESS {
            return Err(());
        }
        OwnedCFType::new(value).ok_or(())
    }

    fn copy_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
        let attribute = cf_string(attribute);
        let value = copy_attribute_value(element, attribute.as_ptr()).ok()?;
        cf_string_to_string(value.as_ptr())
    }

    fn copy_point_attribute(element: AXUIElementRef, attribute: &str) -> Option<CGPoint> {
        let attribute = cf_string(attribute);
        let value = copy_attribute_value(element, attribute.as_ptr()).ok()?;
        let mut point = CGPoint::default();
        let ok = unsafe {
            AXValueGetValue(
                value.as_ptr().cast(),
                K_AX_VALUE_CGPOINT_TYPE,
                (&mut point as *mut CGPoint).cast(),
            )
        };
        if ok {
            Some(point)
        } else {
            None
        }
    }

    fn copy_size_attribute(element: AXUIElementRef, attribute: &str) -> Option<CGSize> {
        let attribute = cf_string(attribute);
        let value = copy_attribute_value(element, attribute.as_ptr()).ok()?;
        let mut size = CGSize::default();
        let ok = unsafe {
            AXValueGetValue(
                value.as_ptr().cast(),
                K_AX_VALUE_CGSIZE_TYPE,
                (&mut size as *mut CGSize).cast(),
            )
        };
        if ok {
            Some(size)
        } else {
            None
        }
    }

    fn cf_string(raw: &str) -> OwnedCFType {
        let bytes = std::ffi::CString::new(raw).expect("cstring");
        let string = unsafe {
            CFStringCreateWithCString(ptr::null(), bytes.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };
        OwnedCFType::new(string.cast()).expect("cfstring")
    }

    fn cf_string_to_string(value: CFTypeRef) -> Option<String> {
        let length = unsafe { CFStringGetLength(value.cast()) };
        let capacity = (length as usize * 4) + 1;
        let mut buffer = vec![0u8; capacity];
        let ok = unsafe {
            CFStringGetCString(
                value.cast(),
                buffer.as_mut_ptr().cast::<c_char>(),
                capacity as CFIndex,
                K_CF_STRING_ENCODING_UTF8,
            )
        };
        if ok == 0 {
            return None;
        }
        let nul = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
        String::from_utf8(buffer[..nul].to_vec()).ok()
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementIsAttributeSettable(
            element: AXUIElementRef,
            attribute: CFStringRef,
            settable: *mut u8,
        ) -> AXError;
        fn AXUIElementSetAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: CFTypeRef,
        ) -> AXError;
        fn AXValueCreate(the_type: AXValueType, value_ptr: *const c_void) -> AXValueRef;
        fn AXValueGetValue(
            value: AXValueRef,
            the_type: AXValueType,
            value_ptr: *mut c_void,
        ) -> bool;
        fn CFRelease(value: CFTypeRef);
        fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
        fn CFArrayGetValueAtIndex(array: CFArrayRef, index: CFIndex) -> *const c_void;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFStringGetLength(the_string: CFStringRef) -> CFIndex;
        fn CFStringGetCString(
            the_string: CFStringRef,
            buffer: *mut c_char,
            buffer_size: CFIndex,
            encoding: u32,
        ) -> u8;
    }
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
                        if let Some(mac_window) = _state.cached_windows.get(pid) {
                            resize_macos_window(mac_window, resolved_width, resolved_height);
                        }
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
        buffer: &WlBuffer,
        event: <WlBuffer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            for window in _state.windows.values_mut() {
                if let Some(buffers) = window.buffers.as_mut() {
                    for slot in &mut buffers.slots {
                        if slot.buffer == *buffer {
                            slot.busy = false;
                        }
                    }
                }
            }
        }
    }
}
