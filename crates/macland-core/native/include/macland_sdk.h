#ifndef MACLAND_SDK_H
#define MACLAND_SDK_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct macland_sdk_capabilities {
    uint8_t renderer_kind;
    uint8_t supports_software_fallback;
    uint8_t supports_fullscreen_host;
    uint8_t supports_windowed_debug;
    uint8_t supports_single_display_session;
    uint8_t supports_multi_display_session;
    uint8_t supports_c_abi;
} macland_sdk_capabilities;

typedef struct macland_sdk_output_descriptor {
    uint32_t width;
    uint32_t height;
    uint32_t scale_factor_milli;
    uint32_t refresh_hz_milli;
} macland_sdk_output_descriptor;

typedef struct macland_sdk_seat_descriptor {
    uint8_t keyboard_present;
    uint8_t pointer_present;
} macland_sdk_seat_descriptor;

typedef struct macland_sdk_session_snapshot {
    macland_sdk_output_descriptor output;
    macland_sdk_seat_descriptor seat;
    uint8_t fullscreen_active;
    uint8_t compositor_running;
} macland_sdk_session_snapshot;

macland_sdk_capabilities macland_sdk_get_capabilities(void);
macland_sdk_session_snapshot macland_sdk_get_default_session_snapshot(void);

#ifdef __cplusplus
}
#endif

#endif
