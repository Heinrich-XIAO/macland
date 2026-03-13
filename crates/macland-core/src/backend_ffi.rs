use crate::backend::{
    BackendCapabilities, BackendEvent, FrameMetadata, OutputDescriptor, RendererKind,
    SeatDescriptor, SessionSnapshot,
};
use std::ptr::NonNull;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MaclandSdkCapabilities {
    renderer_kind: u8,
    supports_software_fallback: u8,
    supports_fullscreen_host: u8,
    supports_windowed_debug: u8,
    supports_single_display_session: u8,
    supports_multi_display_session: u8,
    supports_c_abi: u8,
    supports_event_queue: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MaclandSdkOutputDescriptor {
    width: u32,
    height: u32,
    scale_factor_milli: u32,
    refresh_hz_milli: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MaclandSdkSeatDescriptor {
    keyboard_present: u8,
    pointer_present: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MaclandSdkSessionSnapshot {
    output: MaclandSdkOutputDescriptor,
    seat: MaclandSdkSeatDescriptor,
    fullscreen_active: u8,
    compositor_running: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MaclandSdkEvent {
    kind: u8,
    x: i32,
    y: i32,
    keycode: u16,
    pressed: u8,
    width: u32,
    height: u32,
    stride: u32,
    age: u64,
}

#[repr(C)]
struct MaclandSdkSessionOpaque {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn macland_sdk_get_capabilities() -> MaclandSdkCapabilities;
    fn macland_sdk_get_default_session_snapshot() -> MaclandSdkSessionSnapshot;
    fn macland_sdk_session_create() -> *mut MaclandSdkSessionOpaque;
    fn macland_sdk_session_destroy(session: *mut MaclandSdkSessionOpaque);
    fn macland_sdk_session_get_snapshot(
        session: *mut MaclandSdkSessionOpaque,
    ) -> MaclandSdkSessionSnapshot;
    fn macland_sdk_session_set_compositor_running(
        session: *mut MaclandSdkSessionOpaque,
        compositor_running: u8,
    );
    fn macland_sdk_session_push_pointer_moved(
        session: *mut MaclandSdkSessionOpaque,
        x: i32,
        y: i32,
    );
    fn macland_sdk_session_push_key_changed(
        session: *mut MaclandSdkSessionOpaque,
        keycode: u16,
        pressed: u8,
    );
    fn macland_sdk_session_push_frame_presented(
        session: *mut MaclandSdkSessionOpaque,
        width: u32,
        height: u32,
        stride: u32,
        age: u64,
    );
    fn macland_sdk_session_push_stopped(session: *mut MaclandSdkSessionOpaque);
    fn macland_sdk_session_pop_event(
        session: *mut MaclandSdkSessionOpaque,
        event: *mut MaclandSdkEvent,
    ) -> u8;
}

pub fn sdk_capabilities() -> BackendCapabilities {
    let capabilities = unsafe { macland_sdk_get_capabilities() };
    BackendCapabilities {
        renderer: match capabilities.renderer_kind {
            1 => RendererKind::Metal,
            _ => RendererKind::Software,
        },
        supports_software_fallback: capabilities.supports_software_fallback != 0,
        supports_fullscreen_host: capabilities.supports_fullscreen_host != 0,
        supports_windowed_debug: capabilities.supports_windowed_debug != 0,
        supports_single_display_session: capabilities.supports_single_display_session != 0,
        supports_multi_display_session: capabilities.supports_multi_display_session != 0,
        supports_event_queue: capabilities.supports_event_queue != 0,
        permission_requirements: vec!["accessibility".to_string(), "input-monitoring".to_string()],
        supports_c_abi: capabilities.supports_c_abi != 0,
    }
}

pub fn sdk_default_session_snapshot() -> SessionSnapshot {
    let snapshot = unsafe { macland_sdk_get_default_session_snapshot() };
    SessionSnapshot {
        output: OutputDescriptor {
            id: "main-display".to_string(),
            width: snapshot.output.width,
            height: snapshot.output.height,
            scale_factor_milli: snapshot.output.scale_factor_milli,
            refresh_hz_milli: snapshot.output.refresh_hz_milli,
        },
        seat: SeatDescriptor {
            name: "macland-seat0".to_string(),
            keyboard_present: snapshot.seat.keyboard_present != 0,
            pointer_present: snapshot.seat.pointer_present != 0,
        },
        fullscreen_active: snapshot.fullscreen_active != 0,
        compositor_running: snapshot.compositor_running != 0,
    }
}

pub struct SdkSession {
    raw: NonNull<MaclandSdkSessionOpaque>,
}

impl SdkSession {
    pub fn new() -> Result<Self, String> {
        let raw = unsafe { macland_sdk_session_create() };
        let raw = NonNull::new(raw).ok_or_else(|| "failed to create SDK session".to_string())?;
        Ok(Self { raw })
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let snapshot = unsafe { macland_sdk_session_get_snapshot(self.raw.as_ptr()) };
        map_snapshot(snapshot)
    }

    pub fn set_compositor_running(&mut self, running: bool) {
        unsafe { macland_sdk_session_set_compositor_running(self.raw.as_ptr(), running as u8) };
    }

    pub fn push_event(&mut self, event: BackendEvent) {
        unsafe {
            match event {
                BackendEvent::PointerMoved { x, y } => {
                    macland_sdk_session_push_pointer_moved(self.raw.as_ptr(), x, y);
                }
                BackendEvent::KeyChanged { keycode, pressed } => {
                    macland_sdk_session_push_key_changed(self.raw.as_ptr(), keycode, pressed as u8);
                }
                BackendEvent::FramePresented(frame) => {
                    macland_sdk_session_push_frame_presented(
                        self.raw.as_ptr(),
                        frame.width,
                        frame.height,
                        frame.stride,
                        frame.age,
                    );
                }
                BackendEvent::SessionStopped => {
                    macland_sdk_session_push_stopped(self.raw.as_ptr());
                }
                BackendEvent::SessionStarted(_) => {}
            }
        }
    }

    pub fn pop_event(&mut self) -> Option<BackendEvent> {
        let mut event = MaclandSdkEvent {
            kind: 0,
            x: 0,
            y: 0,
            keycode: 0,
            pressed: 0,
            width: 0,
            height: 0,
            stride: 0,
            age: 0,
        };
        let has_event = unsafe { macland_sdk_session_pop_event(self.raw.as_ptr(), &mut event) };
        if has_event == 0 {
            return None;
        }
        Some(map_event(event))
    }
}

impl Drop for SdkSession {
    fn drop(&mut self) {
        unsafe { macland_sdk_session_destroy(self.raw.as_ptr()) };
    }
}

fn map_snapshot(snapshot: MaclandSdkSessionSnapshot) -> SessionSnapshot {
    SessionSnapshot {
        output: OutputDescriptor {
            id: "main-display".to_string(),
            width: snapshot.output.width,
            height: snapshot.output.height,
            scale_factor_milli: snapshot.output.scale_factor_milli,
            refresh_hz_milli: snapshot.output.refresh_hz_milli,
        },
        seat: SeatDescriptor {
            name: "macland-seat0".to_string(),
            keyboard_present: snapshot.seat.keyboard_present != 0,
            pointer_present: snapshot.seat.pointer_present != 0,
        },
        fullscreen_active: snapshot.fullscreen_active != 0,
        compositor_running: snapshot.compositor_running != 0,
    }
}

fn map_event(event: MaclandSdkEvent) -> BackendEvent {
    match event.kind {
        2 => BackendEvent::PointerMoved {
            x: event.x,
            y: event.y,
        },
        3 => BackendEvent::KeyChanged {
            keycode: event.keycode,
            pressed: event.pressed != 0,
        },
        4 => BackendEvent::FramePresented(FrameMetadata {
            width: event.width,
            height: event.height,
            stride: event.stride,
            age: event.age,
        }),
        _ => BackendEvent::SessionStopped,
    }
}

#[cfg(test)]
mod tests {
    use super::{SdkSession, sdk_capabilities, sdk_default_session_snapshot};
    use crate::backend::{BackendEvent, RendererKind};

    #[test]
    fn reads_capabilities_from_c_sdk() {
        let capabilities = sdk_capabilities();
        assert_eq!(capabilities.renderer, RendererKind::Metal);
        assert!(capabilities.supports_c_abi);
        assert!(capabilities.supports_event_queue);
    }

    #[test]
    fn reads_snapshot_from_c_sdk() {
        let snapshot = sdk_default_session_snapshot();
        assert_eq!(snapshot.output.width, 1728);
        assert!(snapshot.fullscreen_active);
    }

    #[test]
    fn round_trips_session_events() {
        let mut session = SdkSession::new().unwrap();
        session.set_compositor_running(true);
        assert!(session.snapshot().compositor_running);

        session.push_event(BackendEvent::PointerMoved { x: 7, y: 9 });
        session.push_event(BackendEvent::KeyChanged {
            keycode: 42,
            pressed: true,
        });
        assert_eq!(
            session.pop_event(),
            Some(BackendEvent::PointerMoved { x: 7, y: 9 })
        );
        assert_eq!(
            session.pop_event(),
            Some(BackendEvent::KeyChanged {
                keycode: 42,
                pressed: true,
            })
        );
    }
}
