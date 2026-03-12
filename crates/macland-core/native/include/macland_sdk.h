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
    uint8_t supports_event_queue;
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

typedef struct macland_sdk_event {
    uint8_t kind;
    int32_t x;
    int32_t y;
    uint16_t keycode;
    uint8_t pressed;
    uint32_t width;
    uint32_t height;
    uint32_t stride;
    uint64_t age;
} macland_sdk_event;

typedef struct macland_sdk_session macland_sdk_session;

macland_sdk_capabilities macland_sdk_get_capabilities(void);
macland_sdk_session_snapshot macland_sdk_get_default_session_snapshot(void);
macland_sdk_session *macland_sdk_session_create(void);
void macland_sdk_session_destroy(macland_sdk_session *session);
macland_sdk_session_snapshot macland_sdk_session_get_snapshot(macland_sdk_session *session);
void macland_sdk_session_set_compositor_running(macland_sdk_session *session, uint8_t compositor_running);
void macland_sdk_session_push_pointer_moved(macland_sdk_session *session, int32_t x, int32_t y);
void macland_sdk_session_push_key_changed(macland_sdk_session *session, uint16_t keycode, uint8_t pressed);
void macland_sdk_session_push_frame_presented(
    macland_sdk_session *session,
    uint32_t width,
    uint32_t height,
    uint32_t stride,
    uint64_t age
);
void macland_sdk_session_push_stopped(macland_sdk_session *session);
uint8_t macland_sdk_session_pop_event(macland_sdk_session *session, macland_sdk_event *event);

#ifdef __cplusplus
}
#endif

#endif
