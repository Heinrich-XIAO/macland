use serde::Serialize;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, State as ToplevelState, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    self, Flags as ScreencopyFlags, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

const BTN_LEFT: u32 = 0x110;
const KEY_A: u32 = 30;
const KEYMAP_CONTENTS: &str = concat!(
    "xkb_keymap {\n",
    "xkb_keycodes { include \"evdev+aliases(qwerty)\" };\n",
    "xkb_types { include \"complete\" };\n",
    "xkb_compatibility { include \"complete\" };\n",
    "xkb_symbols { include \"pc+us+inet(evdev)\" };\n",
    "xkb_geometry { include \"pc(pc105)\" };\n",
    "};\n\0",
);

fn main() -> ExitCode {
    let options = CliOptions::from_env();
    match run(&options) {
        Ok(report) => {
            if let Some(report_path) = options.report_path.as_ref() {
                if let Some(parent) = report_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let payload = serde_json::to_vec_pretty(&report).unwrap();
                let _ = fs::write(report_path, payload);
            } else {
                println!("{}", serde_json::to_string(&report).unwrap());
            }

            if report.connected
                && report.configured
                && report.first_frame_presented
                && (!options.screenshot_requested() || report.screenshot_captured)
            {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn trace(message: impl AsRef<str>) {
    if env::var_os("MACLAND_REFERENCE_TRACE").is_some() {
        eprintln!("[macland-reference-client] {}", message.as_ref());
    }
}

#[derive(Debug, Default)]
struct CliOptions {
    report_path: Option<PathBuf>,
    screenshot_path: Option<PathBuf>,
}

impl CliOptions {
    fn from_env() -> Self {
        let mut options = Self::default();
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--report-file" => {
                    options.report_path = args.next().map(PathBuf::from);
                }
                "--screenshot-file" => {
                    options.screenshot_path = args.next().map(PathBuf::from);
                }
                _ => {}
            }
        }
        options
    }

    fn screenshot_requested(&self) -> bool {
        self.screenshot_path.is_some()
    }
}

fn run(options: &CliOptions) -> Result<ClientReport, String> {
    trace("connecting to compositor");
    let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
    let (globals, mut event_queue) =
        registry_queue_init(&connection).map_err(|err| err.to_string())?;
    let queue_handle = event_queue.handle();
    trace("binding globals");

    let compositor: WlCompositor = globals
        .bind(&queue_handle, 1..=WlCompositor::interface().version, ())
        .map_err(|err| err.to_string())?;
    let xdg_wm_base: XdgWmBase = globals
        .bind(&queue_handle, 1..=XdgWmBase::interface().version, ())
        .map_err(|err| err.to_string())?;
    let viewporter: Option<WpViewporter> = globals
        .bind(&queue_handle, 1..=WpViewporter::interface().version, ())
        .ok();
    let single_pixel_buffer_manager: Option<WpSinglePixelBufferManagerV1> = globals
        .bind(
            &queue_handle,
            1..=WpSinglePixelBufferManagerV1::interface().version,
            (),
        )
        .ok();
    let shm: WlShm = globals
        .bind(&queue_handle, 1..=WlShm::interface().version, ())
        .map_err(|err| err.to_string())?;
    let seat = globals
        .bind(&queue_handle, 1..=WlSeat::interface().version, ())
        .ok();
    let virtual_pointer_manager = globals
        .bind(
            &queue_handle,
            1..=ZwlrVirtualPointerManagerV1::interface().version,
            (),
        )
        .ok();
    let virtual_keyboard_manager = globals
        .bind(
            &queue_handle,
            1..=ZwpVirtualKeyboardManagerV1::interface().version,
            (),
        )
        .ok();
    let primary_output = bind_first_output(&globals, &queue_handle)?;
    let screencopy_manager = if options.screenshot_requested() {
        Some(
            globals
                .bind(
                    &queue_handle,
                    1..=ZwlrScreencopyManagerV1::interface().version,
                    (),
                )
                .map_err(|err| err.to_string())?,
        )
    } else {
        None
    };

    let surface = compositor.create_surface(&queue_handle, ());
    let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &queue_handle, ());
    let xdg_toplevel = xdg_surface.get_toplevel(&queue_handle, ());
    xdg_toplevel.set_title("macland-reference-client".to_string());
    let viewport = viewporter
        .as_ref()
        .map(|value| value.get_viewport(&surface, &queue_handle, ()));
    surface.commit();
    trace("committed toplevel surface");
    queue_sync(&connection, &queue_handle);
    connection.flush().map_err(|err| err.to_string())?;

    let mut state = ClientState {
        xdg_surface,
        xdg_toplevel,
        surface,
        shm,
        shm_pool: None,
        shm_backing: None,
        viewport,
        single_pixel_buffer_manager,
        seat,
        pointer: None,
        keyboard: None,
        virtual_pointer_manager,
        virtual_pointer: None,
        virtual_keyboard_manager,
        virtual_keyboard: None,
        virtual_keymap: None,
        screenshot: options
            .screenshot_path
            .as_ref()
            .map(|path| ScreenshotState::new(path.clone(), primary_output, screencopy_manager)),
        configured: false,
        first_frame_presented: false,
        keyboard_focus: false,
        pointer_events: 0,
        key_events: 0,
        pending_size: (128, 128),
        pending_states: Vec::new(),
        buffer: None,
        pointer_motion_injected: false,
        pointer_button_injected: false,
        keyboard_key_injected: false,
    };

    let frame_deadline = Instant::now() + Duration::from_secs(5);
    while !state.first_frame_presented && Instant::now() < frame_deadline {
        event_queue.roundtrip(&mut state).map_err(|err| err.to_string())?;
    }
    trace(format!(
        "first frame state configured={} presented={}",
        state.configured, state.first_frame_presented
    ));

    if let Some(screenshot_error) = state.screenshot_error() {
        return Err(screenshot_error);
    }

    if state.screenshot_requested() {
        state.begin_screenshot(&queue_handle);
        trace("requested screencopy");

        let screenshot_deadline = Instant::now() + Duration::from_secs(5);
        while !state.screenshot_complete() && Instant::now() < screenshot_deadline {
            event_queue.roundtrip(&mut state).map_err(|err| err.to_string())?;
        }
        trace(format!(
            "screencopy state ready={} error={}",
            state.screenshot_complete(),
            state.screenshot_error().unwrap_or_else(|| "none".to_string())
        ));

        if let Some(screenshot_error) = state.screenshot_error() {
            return Err(screenshot_error);
        }
        if !state.screenshot_complete() {
            return Err("timed out waiting for screencopy capture".to_string());
        }
    }

    if options.screenshot_requested() {
        return Ok(ClientReport {
            connected: true,
            configured: state.configured,
            first_frame_presented: state.first_frame_presented,
            keyboard_focus: state.keyboard_focus,
            pointer_events: state.pointer_events,
            key_events: state.key_events,
            seat_present: state.seat.is_some(),
            virtual_pointer_supported: state.virtual_pointer_supported(),
            virtual_keyboard_supported: state.virtual_keyboard_supported(),
            pointer_injection_attempted: false,
            keyboard_injection_attempted: false,
            screenshot_captured: state.screenshot_complete(),
        });
    }

    state.maybe_setup_virtual_devices(&queue_handle);

    if state.can_inject_pointer_motion() {
        state.inject_pointer_motion();
        queue_sync(&connection, &queue_handle);
        connection.flush().map_err(|err| err.to_string())?;
    }

    let input_deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < input_deadline && !state.input_verification_complete() {
        if state.can_inject_pointer_button() {
            state.inject_pointer_button();
        }

        if state.can_inject_keyboard_key() {
            state.inject_keyboard_key()?;
        }

        event_queue.roundtrip(&mut state).map_err(|err| err.to_string())?;
    }

    if !state.input_verification_complete()
        && (state.virtual_pointer_supported() || state.virtual_keyboard_supported())
    {
        thread::sleep(Duration::from_millis(100));
    }

    Ok(ClientReport {
        connected: true,
        configured: state.configured,
        first_frame_presented: state.first_frame_presented,
        keyboard_focus: state.keyboard_focus,
        pointer_events: state.pointer_events,
        key_events: state.key_events,
        seat_present: state.seat.is_some(),
        virtual_pointer_supported: state.virtual_pointer_supported(),
        virtual_keyboard_supported: state.virtual_keyboard_supported(),
        pointer_injection_attempted: state.pointer_motion_injected || state.pointer_button_injected,
        keyboard_injection_attempted: state.keyboard_key_injected,
        screenshot_captured: state.screenshot_complete(),
    })
}

fn bind_first_output(
    globals: &wayland_client::globals::GlobalList,
    qh: &QueueHandle<ClientState>,
) -> Result<WlOutput, String> {
    let global = globals
        .contents()
        .clone_list()
        .into_iter()
        .find(|global| global.interface == "wl_output")
        .ok_or_else(|| "no wl_output globals were advertised by the compositor".to_string())?;
    Ok(globals
        .registry()
        .bind(global.name, global.version.min(WlOutput::interface().version), qh, ()))
}

fn queue_sync(connection: &Connection, queue_handle: &QueueHandle<ClientState>) {
    let _ = connection
        .display()
        .sync(queue_handle, CallbackKind::RoundtripBarrier);
}

fn now_millis(start: Instant) -> u32 {
    start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientReport {
    connected: bool,
    configured: bool,
    first_frame_presented: bool,
    keyboard_focus: bool,
    pointer_events: u32,
    key_events: u32,
    seat_present: bool,
    virtual_pointer_supported: bool,
    virtual_keyboard_supported: bool,
    pointer_injection_attempted: bool,
    keyboard_injection_attempted: bool,
    screenshot_captured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallbackKind {
    FramePresentation,
    RoundtripBarrier,
}

struct ClientState {
    xdg_surface: XdgSurface,
    xdg_toplevel: XdgToplevel,
    surface: WlSurface,
    shm: WlShm,
    shm_pool: Option<WlShmPool>,
    shm_backing: Option<File>,
    viewport: Option<WpViewport>,
    single_pixel_buffer_manager: Option<WpSinglePixelBufferManagerV1>,
    seat: Option<WlSeat>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    virtual_pointer_manager: Option<ZwlrVirtualPointerManagerV1>,
    virtual_pointer: Option<ZwlrVirtualPointerV1>,
    virtual_keyboard_manager: Option<ZwpVirtualKeyboardManagerV1>,
    virtual_keyboard: Option<ZwpVirtualKeyboardV1>,
    virtual_keymap: Option<File>,
    screenshot: Option<ScreenshotState>,
    configured: bool,
    first_frame_presented: bool,
    keyboard_focus: bool,
    pointer_events: u32,
    key_events: u32,
    pending_size: (i32, i32),
    pending_states: Vec<ToplevelState>,
    buffer: Option<WlBuffer>,
    pointer_motion_injected: bool,
    pointer_button_injected: bool,
    keyboard_key_injected: bool,
}

struct ScreenshotState {
    path: PathBuf,
    output: WlOutput,
    manager: Option<ZwlrScreencopyManagerV1>,
    frame: Option<ZwlrScreencopyFrameV1>,
    buffer_info: Option<ScreenshotBufferInfo>,
    shm_backing: Option<File>,
    shm_pool: Option<WlShmPool>,
    buffer: Option<WlBuffer>,
    buffer_done: bool,
    copy_requested: bool,
    ready: bool,
    failed: Option<String>,
    y_invert: bool,
}

#[derive(Clone, Copy)]
struct ScreenshotBufferInfo {
    format: wl_shm::Format,
    width: u32,
    height: u32,
    stride: u32,
}

impl ClientState {
    fn screenshot_requested(&self) -> bool {
        self.screenshot.is_some()
    }

    fn screenshot_complete(&self) -> bool {
        self.screenshot.as_ref().is_some_and(|screenshot| screenshot.ready)
    }

    fn screenshot_error(&self) -> Option<String> {
        self.screenshot
            .as_ref()
            .and_then(|screenshot| screenshot.failed.clone())
    }

    fn begin_screenshot(&mut self, queue_handle: &QueueHandle<Self>) {
        let Some(screenshot) = self.screenshot.as_mut() else {
            return;
        };
        if screenshot.copy_requested || screenshot.ready || screenshot.failed.is_some() {
            return;
        }
        let Some(manager) = screenshot.manager.as_ref() else {
            screenshot.failed = Some("screencopy manager was not available".to_string());
            return;
        };
        let frame = manager.capture_output(0, &screenshot.output, queue_handle, ());
        screenshot.frame = Some(frame);
    }

    fn screenshot_matches(&self, frame: &ZwlrScreencopyFrameV1) -> bool {
        self.screenshot
            .as_ref()
            .and_then(|screenshot| screenshot.frame.as_ref())
            .is_some_and(|active| active == frame)
    }

    fn note_screenshot_buffer(
        &mut self,
        format: WEnum<wl_shm::Format>,
        width: u32,
        height: u32,
        stride: u32,
    ) {
        let Some(screenshot) = self.screenshot.as_mut() else {
            return;
        };
        let format = match format {
            WEnum::Value(value) => value,
            WEnum::Unknown(value) => {
                screenshot.failed =
                    Some(format!("unsupported screencopy wl_shm format value {value}"));
                return;
            }
        };
        screenshot.buffer_info = Some(ScreenshotBufferInfo {
            format,
            width,
            height,
            stride,
        });
    }

    fn issue_screenshot_copy(&mut self, queue_handle: &QueueHandle<Self>) {
        let Some(screenshot) = self.screenshot.as_ref() else {
            return;
        };
        if screenshot.copy_requested || screenshot.ready || screenshot.failed.is_some() {
            return;
        }
        let Some(frame) = screenshot.frame.as_ref().cloned() else {
            return;
        };
        let Some(info) = screenshot.buffer_info else {
            return;
        };
        if !screenshot.buffer_done {
            return;
        }

        match self.create_screenshot_buffer(info, queue_handle) {
            Ok((buffer, pool, backing)) => {
                frame.copy(&buffer);
                if let Some(screenshot) = self.screenshot.as_mut() {
                    screenshot.buffer = Some(buffer);
                    screenshot.shm_pool = Some(pool);
                    screenshot.shm_backing = Some(backing);
                    screenshot.copy_requested = true;
                }
            }
            Err(error) => {
                if let Some(screenshot) = self.screenshot.as_mut() {
                    screenshot.failed = Some(error);
                }
            }
        }
    }

    fn create_screenshot_buffer(
        &mut self,
        info: ScreenshotBufferInfo,
        queue_handle: &QueueHandle<Self>,
    ) -> Result<(WlBuffer, WlShmPool, File), String> {
        let size = (info.stride as usize) * (info.height as usize);
        let backing = create_shm_file(size)?;
        let pool = self
            .shm
            .create_pool(backing.as_fd(), size as i32, queue_handle, ());
        let buffer = pool.create_buffer(
            0,
            info.width as i32,
            info.height as i32,
            info.stride as i32,
            info.format,
            queue_handle,
            (),
        );
        Ok((buffer, pool, backing))
    }

    fn note_screenshot_flags(&mut self, flags: WEnum<ScreencopyFlags>) {
        let Some(screenshot) = self.screenshot.as_mut() else {
            return;
        };
        screenshot.y_invert = matches!(flags, WEnum::Value(value) if value.contains(ScreencopyFlags::YInvert));
    }

    fn finalize_screenshot(&mut self) {
        let Some(screenshot) = self.screenshot.as_mut() else {
            return;
        };
        let result = screenshot.write_png();
        if let Some(frame) = screenshot.frame.take() {
            frame.destroy();
        }
        match result {
            Ok(()) => screenshot.ready = true,
            Err(error) => screenshot.failed = Some(error),
        }
    }

    fn fail_screenshot(&mut self, message: impl Into<String>) {
        if let Some(screenshot) = self.screenshot.as_mut() {
            screenshot.failed = Some(message.into());
            if let Some(frame) = screenshot.frame.take() {
                frame.destroy();
            }
        }
    }

    fn attach_frame(&mut self, queue_handle: &QueueHandle<Self>) {
        let width = self.pending_size.0.max(1);
        let height = self.pending_size.1.max(1);
        let use_single_pixel_buffer = !self.screenshot_requested();
        let buffer = if use_single_pixel_buffer && let (Some(viewport), Some(single_pixel_buffer_manager)) = (
            self.viewport.as_ref(),
            self.single_pixel_buffer_manager.as_ref(),
        ) {
            let buffer =
                single_pixel_buffer_manager.create_u32_rgba_buffer(0x2f30, 0x8f8f, 0xeeee, u32::MAX, queue_handle, ());
            viewport.set_destination(width, height);
            buffer
        } else {
            match self.create_shm_buffer(width, height, queue_handle) {
                Ok(buffer) => buffer,
                Err(_) => return,
            }
        };
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface
            .frame(queue_handle, CallbackKind::FramePresentation);
        self.surface.commit();
        self.buffer = Some(buffer);
    }

    fn create_shm_buffer(
        &mut self,
        width: i32,
        height: i32,
        queue_handle: &QueueHandle<Self>,
    ) -> Result<WlBuffer, String> {
        let stride = width * 4;
        let size = stride * height;
        let mut backing = create_shm_file(size as usize)?;
        draw_solid_buffer(&mut backing, width as usize, height as usize)?;
        let pool = self
            .shm
            .create_pool(backing.as_fd(), size, queue_handle, ());
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            stride,
            wl_shm::Format::Argb8888,
            queue_handle,
            (),
        );
        self.shm_backing = Some(backing);
        self.shm_pool = Some(pool);
        Ok(buffer)
    }

    fn maybe_setup_virtual_devices(&mut self, queue_handle: &QueueHandle<Self>) {
        if self.virtual_pointer.is_none() {
            if let (Some(manager), Some(seat)) =
                (self.virtual_pointer_manager.as_ref(), self.seat.as_ref())
            {
                let pointer = manager.create_virtual_pointer(Some(seat), queue_handle, ());
                self.virtual_pointer = Some(pointer);
            }
        }

        if self.virtual_keyboard.is_none() {
            if let (Some(manager), Some(seat)) =
                (self.virtual_keyboard_manager.as_ref(), self.seat.as_ref())
            {
                let keyboard = manager.create_virtual_keyboard(seat, queue_handle, ());
                self.virtual_keyboard = Some(keyboard);
            }
        }
    }

    fn virtual_pointer_supported(&self) -> bool {
        self.virtual_pointer_manager.is_some() && self.seat.is_some()
    }

    fn virtual_keyboard_supported(&self) -> bool {
        self.virtual_keyboard_manager.is_some() && self.seat.is_some()
    }

    fn can_inject_pointer_motion(&self) -> bool {
        self.virtual_pointer.is_some() && !self.pointer_motion_injected
    }

    fn can_inject_pointer_button(&self) -> bool {
        self.virtual_pointer.is_some()
            && self.pointer_motion_injected
            && !self.pointer_button_injected
            && self.pointer_events > 0
    }

    fn can_inject_keyboard_key(&self) -> bool {
        self.virtual_keyboard.is_some() && !self.keyboard_key_injected && self.keyboard_focus
    }

    fn inject_pointer_motion(&mut self) {
        if let Some(pointer) = self.virtual_pointer.as_ref() {
            let now = now_millis(Instant::now());
            let width = self.pending_size.0.max(1) as u32;
            let height = self.pending_size.1.max(1) as u32;
            pointer.motion_absolute(now, width / 2, height / 2, width, height);
            pointer.frame();
            self.pointer_motion_injected = true;
        }
    }

    fn inject_pointer_button(&mut self) {
        if let Some(pointer) = self.virtual_pointer.as_ref() {
            let now = now_millis(Instant::now());
            pointer.button(now, BTN_LEFT, wl_pointer::ButtonState::Pressed);
            pointer.button(now, BTN_LEFT, wl_pointer::ButtonState::Released);
            pointer.frame();
            self.pointer_button_injected = true;
        }
    }

    fn inject_keyboard_key(&mut self) -> Result<(), String> {
        let Some(keyboard) = self.virtual_keyboard.as_ref() else {
            return Ok(());
        };

        if self.virtual_keymap.is_none() {
            self.virtual_keymap = Some(create_keymap_file()?);
        }
        let keymap_file = self.virtual_keymap.as_mut().unwrap();
        keymap_file
            .seek(SeekFrom::Start(0))
            .map_err(|err| err.to_string())?;
        keyboard.keymap(
            wl_keyboard::KeymapFormat::XkbV1.into(),
            keymap_file.as_fd(),
            KEYMAP_CONTENTS.len() as u32,
        );
        keyboard.modifiers(0, 0, 0, 0);
        let now = now_millis(Instant::now());
        keyboard.key(now, KEY_A, wl_keyboard::KeyState::Pressed.into());
        keyboard.key(now, KEY_A, wl_keyboard::KeyState::Released.into());
        self.keyboard_key_injected = true;
        Ok(())
    }

    fn input_verification_complete(&self) -> bool {
        let pointer_ok = !self.virtual_pointer_supported() || self.pointer_events > 0;
        let keyboard_ok = !self.virtual_keyboard_supported() || self.key_events > 0;
        pointer_ok && keyboard_ok
    }
}

impl ScreenshotState {
    fn new(
        path: PathBuf,
        output: WlOutput,
        manager: Option<ZwlrScreencopyManagerV1>,
    ) -> Self {
        Self {
            path,
            output,
            manager,
            frame: None,
            buffer_info: None,
            shm_backing: None,
            shm_pool: None,
            buffer: None,
            buffer_done: false,
            copy_requested: false,
            ready: false,
            failed: None,
            y_invert: false,
        }
    }

    fn write_png(&mut self) -> Result<(), String> {
        let info = self
            .buffer_info
            .ok_or_else(|| "screencopy did not advertise any wl_shm buffer information".to_string())?;
        let mut backing = self
            .shm_backing
            .as_ref()
            .ok_or_else(|| "screencopy shm backing file was missing".to_string())?
            .try_clone()
            .map_err(|err| err.to_string())?;
        backing.seek(SeekFrom::Start(0)).map_err(|err| err.to_string())?;
        let mut raw = vec![0; info.stride as usize * info.height as usize];
        backing.read_exact(&mut raw).map_err(|err| err.to_string())?;
        let rgba = convert_screencopy_to_rgba(&raw, info, self.y_invert)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        write_png_rgba(&self.path, info.width, info.height, &rgba)
    }
}

fn convert_screencopy_to_rgba(
    raw: &[u8],
    info: ScreenshotBufferInfo,
    y_invert: bool,
) -> Result<Vec<u8>, String> {
    let width = info.width as usize;
    let height = info.height as usize;
    let stride = info.stride as usize;
    let mut rgba = vec![0; width * height * 4];
    for row in 0..height {
        let source_row = if y_invert { height - 1 - row } else { row };
        let source = &raw[source_row * stride..source_row * stride + width * 4];
        let destination = &mut rgba[row * width * 4..(row + 1) * width * 4];
        for (src, dst) in source.chunks_exact(4).zip(destination.chunks_exact_mut(4)) {
            let (r, g, b, a) = match info.format {
                wl_shm::Format::Argb8888 => (src[2], src[1], src[0], src[3]),
                wl_shm::Format::Xrgb8888 => (src[2], src[1], src[0], 0xFF),
                other => {
                    return Err(format!(
                        "unsupported screencopy pixel format: {:?}",
                        other
                    ))
                }
            };
            dst.copy_from_slice(&[r, g, b, a]);
        }
    }
    Ok(rgba)
}

fn write_png_rgba(path: &PathBuf, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!(
            "invalid RGBA payload size {}, expected {expected}",
            rgba.len()
        ));
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    append_png_chunk(&mut png, *b"IHDR", &ihdr);

    let mut raw_scanlines = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    let row_len = width as usize * 4;
    for row in rgba.chunks_exact(row_len) {
        raw_scanlines.push(0);
        raw_scanlines.extend_from_slice(row);
    }

    let idat = encode_zlib_store_blocks(&raw_scanlines);
    append_png_chunk(&mut png, *b"IDAT", &idat);
    append_png_chunk(&mut png, *b"IEND", &[]);

    fs::write(path, png).map_err(|err| err.to_string())
}

fn append_png_chunk(target: &mut Vec<u8>, chunk_type: [u8; 4], payload: &[u8]) {
    target.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    target.extend_from_slice(&chunk_type);
    target.extend_from_slice(payload);

    let mut crc_input = Vec::with_capacity(chunk_type.len() + payload.len());
    crc_input.extend_from_slice(&chunk_type);
    crc_input.extend_from_slice(payload);
    target.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn encode_zlib_store_blocks(payload: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(payload.len() + payload.len() / 65535 * 5 + 6);
    encoded.extend_from_slice(&[0x78, 0x01]);

    let mut offset = 0usize;
    while offset < payload.len() {
        let remaining = payload.len() - offset;
        let block_len = remaining.min(65_535);
        let final_block = usize::from(offset + block_len == payload.len()) as u8;
        encoded.push(final_block);
        encoded.extend_from_slice(&(block_len as u16).to_le_bytes());
        encoded.extend_from_slice((!(block_len as u16)).to_le_bytes().as_slice());
        encoded.extend_from_slice(&payload[offset..offset + block_len]);
        offset += block_len;
    }

    encoded.extend_from_slice(&adler32(payload).to_be_bytes());
    encoded
}

fn adler32(bytes: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in bytes {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg() & 0xEDB8_8320;
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

fn create_keymap_file() -> Result<File, String> {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let root = PathBuf::from(runtime_dir);
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    for attempt in 0..32 {
        let path = root.join(format!(
            "macland-reference-keymap-{}-{attempt}.xkb",
            std::process::id()
        ));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(KEYMAP_CONTENTS.as_bytes())
                    .map_err(|err| err.to_string())?;
                file.seek(SeekFrom::Start(0))
                    .map_err(|err| err.to_string())?;
                return Ok(file);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.to_string()),
        }
    }

    Err("failed to allocate virtual keyboard keymap file".to_string())
}

fn create_shm_file(size: usize) -> Result<File, String> {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let root = PathBuf::from(runtime_dir);
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    for attempt in 0..32 {
        let path = root.join(format!(
            "macland-reference-shm-{}-{attempt}.bin",
            std::process::id()
        ));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                file.set_len(size as u64).map_err(|err| err.to_string())?;
                return Ok(file);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.to_string()),
        }
    }

    Err("failed to allocate shm backing file".to_string())
}

fn draw_solid_buffer(file: &mut File, width: usize, height: usize) -> Result<(), String> {
    let size = width * height * 4;
    let mut pixels = vec![0; size];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[0x20, 0x40, 0x60, 0xFF]);
    }
    file.seek(SeekFrom::Start(0)).map_err(|err| err.to_string())?;
    file.write_all(&pixels).map_err(|err| err.to_string())?;
    file.seek(SeekFrom::Start(0)).map_err(|err| err.to_string())?;
    Ok(())
}

impl Dispatch<WlCompositor, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlCompositor,
        _event: <WlCompositor as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgWmBase, ()> for ClientState {
    fn event(
        _state: &mut Self,
        xdg_wm_base: &XdgWmBase,
        event: <XdgWmBase as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            xdg_wm_base.pong(serial);
        }
    }
}

impl Dispatch<WlSurface, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSurface,
        _event: <WlSurface as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgSurface, ()> for ClientState {
    fn event(
        state: &mut Self,
        xdg_surface: &XdgSurface,
        event: <XdgSurface as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if xdg_surface != &state.xdg_surface {
            return;
        }

        if let xdg_surface::Event::Configure { serial } = event {
            trace("received xdg_surface configure");
            state.configured = true;
            state.xdg_surface.ack_configure(serial);
            state.attach_frame(qh);
        }
    }
}

impl Dispatch<XdgToplevel, ()> for ClientState {
    fn event(
        state: &mut Self,
        xdg_toplevel: &XdgToplevel,
        event: <XdgToplevel as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if xdg_toplevel != &state.xdg_toplevel {
            return;
        }

        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                state.pending_size = if state.screenshot_requested() {
                    (width.clamp(320, 720), height.clamp(220, 480))
                } else {
                    (width.max(128), height.max(128))
                };
                state.pending_states = states
                    .chunks_exact(4)
                    .filter_map(|chunk| <[u8; 4]>::try_from(chunk).ok())
                    .map(u32::from_ne_bytes)
                    .filter_map(|value| ToplevelState::try_from(value).ok())
                    .collect();
                state.keyboard_focus = state
                    .pending_states
                    .iter()
                    .any(|value| *value == ToplevelState::Activated);
            }
            xdg_toplevel::Event::Close => {
                state.first_frame_presented = true;
            }
            xdg_toplevel::Event::ConfigureBounds { width, height } => {
                if width > 0 && height > 0 {
                    state.pending_size = (width, height);
                }
            }
            xdg_toplevel::Event::WmCapabilities { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for ClientState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Capabilities {
                capabilities: WEnum::Value(capabilities),
            } => {
                if capabilities.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                    state.pointer = Some(seat.get_pointer(qh, ()));
                }
                if capabilities.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none()
                {
                    state.keyboard = Some(seat.get_keyboard(qh, ()));
                }
                state.maybe_setup_virtual_devices(qh);
            }
            wl_seat::Event::Name { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlOutput,
        _event: <WlOutput as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlPointer, ()> for ClientState {
    fn event(
        state: &mut Self,
        _pointer: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { .. }
            | wl_pointer::Event::Motion { .. }
            | wl_pointer::Event::Button { .. } => {
                state.pointer_events += 1;
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for ClientState {
    fn event(
        state: &mut Self,
        _keyboard: &WlKeyboard,
        event: <WlKeyboard as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { .. } => {
                state.keyboard_focus = true;
            }
            wl_keyboard::Event::Key {
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                state.key_events += 1;
            }
            wl_keyboard::Event::Keymap { .. }
            | wl_keyboard::Event::Leave { .. }
            | wl_keyboard::Event::Modifiers { .. }
            | wl_keyboard::Event::RepeatInfo { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<WpViewporter, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewporter,
        _event: <WpViewporter as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<WpViewport, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewport,
        _event: <WpViewport as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<WpSinglePixelBufferManagerV1, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpSinglePixelBufferManagerV1,
        _event: <WpSinglePixelBufferManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<WlShm, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShm,
        _event: <WlShm as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_buffer::Event::Release => {}
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlCallback, CallbackKind> for ClientState {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        data: &CallbackKind,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_callback::Event::Done { .. } => {
                if *data == CallbackKind::FramePresentation {
                    trace("received frame callback");
                    state.first_frame_presented = true;
                }
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if !state.screenshot_matches(frame) {
            return;
        }

        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                trace(format!("screencopy buffer {width}x{height} stride={stride}"));
                state.note_screenshot_buffer(format, width, height, stride);
            }
            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                trace("screencopy flags received");
                state.note_screenshot_flags(flags);
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                trace("screencopy buffer_done");
                if let Some(screenshot) = state.screenshot.as_mut() {
                    screenshot.buffer_done = true;
                }
                state.issue_screenshot_copy(qh);
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                trace("screencopy ready");
                state.finalize_screenshot();
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                trace("screencopy failed");
                state.fail_screenshot("compositor rejected the screencopy request");
            }
            zwlr_screencopy_frame_v1::Event::Damage { .. }
            | zwlr_screencopy_frame_v1::Event::LinuxDmabuf { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerManagerV1,
        _event: <ZwlrVirtualPointerManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerV1,
        _event: <ZwlrVirtualPointerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: <ZwpVirtualKeyboardManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for ClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: <ZwpVirtualKeyboardV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}
