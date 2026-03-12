#include "macland_sdk.h"

#include <stdlib.h>
#include <string.h>

#define MACLAND_SDK_EVENT_QUEUE_CAPACITY 64

struct macland_sdk_session {
    macland_sdk_session_snapshot snapshot;
    macland_sdk_event events[MACLAND_SDK_EVENT_QUEUE_CAPACITY];
    uint32_t head;
    uint32_t tail;
    uint32_t len;
};

static void macland_sdk_enqueue_event(macland_sdk_session *session, macland_sdk_event event) {
    if (session == NULL) {
        return;
    }

    if (session->len == MACLAND_SDK_EVENT_QUEUE_CAPACITY) {
        session->head = (session->head + 1U) % MACLAND_SDK_EVENT_QUEUE_CAPACITY;
        session->len -= 1U;
    }

    session->events[session->tail] = event;
    session->tail = (session->tail + 1U) % MACLAND_SDK_EVENT_QUEUE_CAPACITY;
    session->len += 1U;
}

macland_sdk_capabilities macland_sdk_get_capabilities(void) {
    macland_sdk_capabilities capabilities;
    capabilities.renderer_kind = 1;
    capabilities.supports_software_fallback = 1;
    capabilities.supports_fullscreen_host = 1;
    capabilities.supports_windowed_debug = 1;
    capabilities.supports_single_display_session = 1;
    capabilities.supports_multi_display_session = 0;
    capabilities.supports_c_abi = 1;
    capabilities.supports_event_queue = 1;
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

macland_sdk_session *macland_sdk_session_create(void) {
    macland_sdk_session *session =
        (macland_sdk_session *)calloc(1U, sizeof(macland_sdk_session));
    if (session == NULL) {
        return NULL;
    }
    session->snapshot = macland_sdk_get_default_session_snapshot();
    return session;
}

void macland_sdk_session_destroy(macland_sdk_session *session) {
    free(session);
}

macland_sdk_session_snapshot macland_sdk_session_get_snapshot(macland_sdk_session *session) {
    if (session == NULL) {
        return macland_sdk_get_default_session_snapshot();
    }
    return session->snapshot;
}

void macland_sdk_session_set_compositor_running(
    macland_sdk_session *session,
    uint8_t compositor_running
) {
    if (session == NULL) {
        return;
    }
    session->snapshot.compositor_running = compositor_running;
}

void macland_sdk_session_push_pointer_moved(macland_sdk_session *session, int32_t x, int32_t y) {
    macland_sdk_event event;
    memset(&event, 0, sizeof(event));
    event.kind = 2;
    event.x = x;
    event.y = y;
    macland_sdk_enqueue_event(session, event);
}

void macland_sdk_session_push_key_changed(
    macland_sdk_session *session,
    uint16_t keycode,
    uint8_t pressed
) {
    macland_sdk_event event;
    memset(&event, 0, sizeof(event));
    event.kind = 3;
    event.keycode = keycode;
    event.pressed = pressed;
    macland_sdk_enqueue_event(session, event);
}

void macland_sdk_session_push_frame_presented(
    macland_sdk_session *session,
    uint32_t width,
    uint32_t height,
    uint32_t stride,
    uint64_t age
) {
    macland_sdk_event event;
    memset(&event, 0, sizeof(event));
    event.kind = 4;
    event.width = width;
    event.height = height;
    event.stride = stride;
    event.age = age;
    macland_sdk_enqueue_event(session, event);
}

void macland_sdk_session_push_stopped(macland_sdk_session *session) {
    macland_sdk_event event;
    memset(&event, 0, sizeof(event));
    event.kind = 5;
    if (session != NULL) {
        session->snapshot.compositor_running = 0;
    }
    macland_sdk_enqueue_event(session, event);
}

uint8_t macland_sdk_session_pop_event(macland_sdk_session *session, macland_sdk_event *event) {
    if (session == NULL || event == NULL || session->len == 0U) {
        return 0;
    }

    *event = session->events[session->head];
    session->head = (session->head + 1U) % MACLAND_SDK_EVENT_QUEUE_CAPACITY;
    session->len -= 1U;
    return 1;
}
