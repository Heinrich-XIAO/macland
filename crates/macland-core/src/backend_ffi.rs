use crate::backend::{
    BackendCapabilities, OutputDescriptor, RendererKind, SeatDescriptor, SessionSnapshot,
};

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

unsafe extern "C" {
    fn macland_sdk_get_capabilities() -> MaclandSdkCapabilities;
    fn macland_sdk_get_default_session_snapshot() -> MaclandSdkSessionSnapshot;
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
        permission_requirements: vec![
            "accessibility".to_string(),
            "input-monitoring".to_string(),
        ],
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

#[cfg(test)]
mod tests {
    use super::{sdk_capabilities, sdk_default_session_snapshot};
    use crate::backend::RendererKind;

    #[test]
    fn reads_capabilities_from_c_sdk() {
        let capabilities = sdk_capabilities();
        assert_eq!(capabilities.renderer, RendererKind::Metal);
        assert!(capabilities.supports_c_abi);
    }

    #[test]
    fn reads_snapshot_from_c_sdk() {
        let snapshot = sdk_default_session_snapshot();
        assert_eq!(snapshot.output.width, 1728);
        assert!(snapshot.fullscreen_active);
    }
}
