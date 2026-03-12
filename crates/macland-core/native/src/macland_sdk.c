#include "macland_sdk.h"

macland_sdk_capabilities macland_sdk_get_capabilities(void) {
    macland_sdk_capabilities capabilities;
    capabilities.renderer_kind = 1;
    capabilities.supports_software_fallback = 1;
    capabilities.supports_fullscreen_host = 1;
    capabilities.supports_windowed_debug = 1;
    capabilities.supports_single_display_session = 1;
    capabilities.supports_multi_display_session = 0;
    capabilities.supports_c_abi = 1;
    return capabilities;
}

macland_sdk_session_snapshot macland_sdk_get_default_session_snapshot(void) {
    macland_sdk_session_snapshot snapshot;
    snapshot.output.width = 1728;
    snapshot.output.height = 1117;
    snapshot.output.scale_factor_milli = 2000;
    snapshot.output.refresh_hz_milli = 60000;
    snapshot.seat.keyboard_present = 1;
    snapshot.seat.pointer_present = 1;
    snapshot.fullscreen_active = 1;
    snapshot.compositor_running = 0;
    return snapshot;
}
