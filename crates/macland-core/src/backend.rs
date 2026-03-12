use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub renderer: RendererKind,
    pub supports_software_fallback: bool,
    pub supports_fullscreen_host: bool,
    pub supports_windowed_debug: bool,
    pub supports_single_display_session: bool,
    pub supports_multi_display_session: bool,
    pub supports_c_abi: bool,
    pub permission_requirements: Vec<String>,
}

impl BackendCapabilities {
    pub fn macos_defaults() -> Self {
        Self {
            renderer: RendererKind::Metal,
            supports_software_fallback: true,
            supports_fullscreen_host: true,
            supports_windowed_debug: true,
            supports_single_display_session: true,
            supports_multi_display_session: false,
            supports_c_abi: false,
            permission_requirements: vec![
                "accessibility".to_string(),
                "input-monitoring".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RendererKind {
    Metal,
    Software,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDescriptor {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor_milli: u32,
    pub refresh_hz_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatDescriptor {
    pub name: String,
    pub keyboard_present: bool,
    pub pointer_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub output: OutputDescriptor,
    pub seat: SeatDescriptor,
    pub fullscreen_active: bool,
    pub compositor_running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameMetadata {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub age: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendEvent {
    SessionStarted(SessionSnapshot),
    PointerMoved { x: i32, y: i32 },
    KeyChanged { keycode: u16, pressed: bool },
    FramePresented(FrameMetadata),
    SessionStopped,
}

pub trait BackendRuntime {
    fn capabilities(&self) -> &BackendCapabilities;
    fn snapshot(&self) -> SessionSnapshot;
    fn push_event(&mut self, event: BackendEvent);
    fn pop_event(&mut self) -> Option<BackendEvent>;
}

#[derive(Debug, Clone)]
pub struct MockBackendRuntime {
    capabilities: BackendCapabilities,
    snapshot: SessionSnapshot,
    events: VecDeque<BackendEvent>,
}

impl MockBackendRuntime {
    pub fn new(snapshot: SessionSnapshot) -> Self {
        Self {
            capabilities: BackendCapabilities::macos_defaults(),
            snapshot,
            events: VecDeque::new(),
        }
    }
}

impl BackendRuntime for MockBackendRuntime {
    fn capabilities(&self) -> &BackendCapabilities {
        &self.capabilities
    }

    fn snapshot(&self) -> SessionSnapshot {
        self.snapshot.clone()
    }

    fn push_event(&mut self, event: BackendEvent) {
        self.events.push_back(event);
    }

    fn pop_event(&mut self) -> Option<BackendEvent> {
        self.events.pop_front()
    }
}

pub fn default_session_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        output: OutputDescriptor {
            id: "main-display".to_string(),
            width: 1728,
            height: 1117,
            scale_factor_milli: 2000,
            refresh_hz_milli: 60000,
        },
        seat: SeatDescriptor {
            name: "macland-seat0".to_string(),
            keyboard_present: true,
            pointer_present: true,
        },
        fullscreen_active: true,
        compositor_running: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_session_snapshot, BackendEvent, BackendRuntime, FrameMetadata, MockBackendRuntime,
        RendererKind,
    };

    #[test]
    fn exposes_default_capabilities() {
        let runtime = MockBackendRuntime::new(default_session_snapshot());
        let capabilities = runtime.capabilities();
        assert_eq!(capabilities.renderer, RendererKind::Metal);
        assert!(capabilities.supports_software_fallback);
        assert!(!capabilities.supports_multi_display_session);
    }

    #[test]
    fn records_events_in_order() {
        let mut runtime = MockBackendRuntime::new(default_session_snapshot());
        runtime.push_event(BackendEvent::PointerMoved { x: 10, y: 20 });
        runtime.push_event(BackendEvent::FramePresented(FrameMetadata {
            width: 100,
            height: 50,
            stride: 400,
            age: 1,
        }));

        assert_eq!(
            runtime.pop_event(),
            Some(BackendEvent::PointerMoved { x: 10, y: 20 })
        );
        assert!(matches!(
            runtime.pop_event(),
            Some(BackendEvent::FramePresented(_))
        ));
    }
}
