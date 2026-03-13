use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FILES: &[(&str, &str)] = &[
    (
        "include/drm_fourcc.h",
        r#"#ifndef DRM_FOURCC_H
#define DRM_FOURCC_H

#include <stdint.h>

#define fourcc_code(a, b, c, d) \
    ((uint32_t)(a) | ((uint32_t)(b) << 8) | ((uint32_t)(c) << 16) | ((uint32_t)(d) << 24))

#define DRM_FORMAT_INVALID 0

#define DRM_FORMAT_C8 fourcc_code('C', '8', ' ', ' ')
#define DRM_FORMAT_R8 fourcc_code('R', '8', ' ', ' ')
#define DRM_FORMAT_GR88 fourcc_code('G', 'R', '8', '8')
#define DRM_FORMAT_RGB332 fourcc_code('R', 'G', 'B', '8')
#define DRM_FORMAT_BGR233 fourcc_code('B', 'G', 'R', '8')
#define DRM_FORMAT_XRGB4444 fourcc_code('X', 'R', '1', '2')
#define DRM_FORMAT_XBGR4444 fourcc_code('X', 'B', '1', '2')
#define DRM_FORMAT_RGBX4444 fourcc_code('R', 'X', '1', '2')
#define DRM_FORMAT_BGRX4444 fourcc_code('B', 'X', '1', '2')
#define DRM_FORMAT_ARGB4444 fourcc_code('A', 'R', '1', '2')
#define DRM_FORMAT_ABGR4444 fourcc_code('A', 'B', '1', '2')
#define DRM_FORMAT_RGBA4444 fourcc_code('R', 'A', '1', '2')
#define DRM_FORMAT_BGRA4444 fourcc_code('B', 'A', '1', '2')
#define DRM_FORMAT_XRGB1555 fourcc_code('X', 'R', '1', '5')
#define DRM_FORMAT_XBGR1555 fourcc_code('X', 'B', '1', '5')
#define DRM_FORMAT_RGBX5551 fourcc_code('R', 'X', '1', '5')
#define DRM_FORMAT_BGRX5551 fourcc_code('B', 'X', '1', '5')
#define DRM_FORMAT_ARGB1555 fourcc_code('A', 'R', '1', '5')
#define DRM_FORMAT_ABGR1555 fourcc_code('A', 'B', '1', '5')
#define DRM_FORMAT_RGBA5551 fourcc_code('R', 'A', '1', '5')
#define DRM_FORMAT_BGRA5551 fourcc_code('B', 'A', '1', '5')
#define DRM_FORMAT_RGB565 fourcc_code('R', 'G', '1', '6')
#define DRM_FORMAT_BGR565 fourcc_code('B', 'G', '1', '6')
#define DRM_FORMAT_RGB888 fourcc_code('R', 'G', '2', '4')
#define DRM_FORMAT_BGR888 fourcc_code('B', 'G', '2', '4')
#define DRM_FORMAT_XRGB8888 fourcc_code('X', 'R', '2', '4')
#define DRM_FORMAT_XBGR8888 fourcc_code('X', 'B', '2', '4')
#define DRM_FORMAT_RGBX8888 fourcc_code('R', 'X', '2', '4')
#define DRM_FORMAT_BGRX8888 fourcc_code('B', 'X', '2', '4')
#define DRM_FORMAT_ARGB8888 fourcc_code('A', 'R', '2', '4')
#define DRM_FORMAT_ABGR8888 fourcc_code('A', 'B', '2', '4')
#define DRM_FORMAT_RGBA8888 fourcc_code('R', 'A', '2', '4')
#define DRM_FORMAT_BGRA8888 fourcc_code('B', 'A', '2', '4')
#define DRM_FORMAT_XRGB2101010 fourcc_code('X', 'R', '3', '0')
#define DRM_FORMAT_XBGR2101010 fourcc_code('X', 'B', '3', '0')
#define DRM_FORMAT_RGBX1010102 fourcc_code('R', 'X', '3', '0')
#define DRM_FORMAT_BGRX1010102 fourcc_code('B', 'X', '3', '0')
#define DRM_FORMAT_ARGB2101010 fourcc_code('A', 'R', '3', '0')
#define DRM_FORMAT_ABGR2101010 fourcc_code('A', 'B', '3', '0')
#define DRM_FORMAT_RGBA1010102 fourcc_code('R', 'A', '3', '0')
#define DRM_FORMAT_BGRA1010102 fourcc_code('B', 'A', '3', '0')
#define DRM_FORMAT_XBGR16161616F fourcc_code('X', 'B', '4', 'H')
#define DRM_FORMAT_ABGR16161616F fourcc_code('A', 'B', '4', 'H')
#define DRM_FORMAT_XBGR16161616 fourcc_code('X', 'B', '4', '8')
#define DRM_FORMAT_ABGR16161616 fourcc_code('A', 'B', '4', '8')
#define DRM_FORMAT_YVYU fourcc_code('Y', 'V', 'Y', 'U')
#define DRM_FORMAT_VYUY fourcc_code('V', 'Y', 'U', 'Y')
#define DRM_FORMAT_NV12 fourcc_code('N', 'V', '1', '2')
#define DRM_FORMAT_NV15 fourcc_code('N', 'V', '1', '5')
#define DRM_FORMAT_NV16 fourcc_code('N', 'V', '1', '6')
#define DRM_FORMAT_NV20 fourcc_code('N', 'V', '2', '0')
#define DRM_FORMAT_NV21 fourcc_code('N', 'V', '2', '1')
#define DRM_FORMAT_NV24 fourcc_code('N', 'V', '2', '4')
#define DRM_FORMAT_NV30 fourcc_code('N', 'V', '3', '0')
#define DRM_FORMAT_NV42 fourcc_code('N', 'V', '4', '2')
#define DRM_FORMAT_NV61 fourcc_code('N', 'V', '6', '1')
#define DRM_FORMAT_P010 fourcc_code('P', '0', '1', '0')
#define DRM_FORMAT_P012 fourcc_code('P', '0', '1', '2')
#define DRM_FORMAT_P016 fourcc_code('P', '0', '1', '6')
#define DRM_FORMAT_P030 fourcc_code('P', '0', '3', '0')
#define DRM_FORMAT_P210 fourcc_code('P', '2', '1', '0')
#define DRM_FORMAT_Q401 fourcc_code('Q', '4', '0', '1')
#define DRM_FORMAT_Q410 fourcc_code('Q', '4', '1', '0')
#define DRM_FORMAT_S010 fourcc_code('S', '0', '1', '0')
#define DRM_FORMAT_S012 fourcc_code('S', '0', '1', '2')
#define DRM_FORMAT_S016 fourcc_code('S', '0', '1', '6')
#define DRM_FORMAT_S210 fourcc_code('S', '2', '1', '0')
#define DRM_FORMAT_S212 fourcc_code('S', '2', '1', '2')
#define DRM_FORMAT_S216 fourcc_code('S', '2', '1', '6')
#define DRM_FORMAT_S410 fourcc_code('S', '4', '1', '0')
#define DRM_FORMAT_S412 fourcc_code('S', '4', '1', '2')
#define DRM_FORMAT_S416 fourcc_code('S', '4', '1', '6')
#define DRM_FORMAT_R16F fourcc_code('R', '1', '6', 'H')
#define DRM_FORMAT_R32F fourcc_code('R', '3', '2', 'F')
#define DRM_FORMAT_GR1616F fourcc_code('G', 'R', '2', 'H')
#define DRM_FORMAT_GR3232F fourcc_code('G', 'R', '2', 'F')
#define DRM_FORMAT_BGR161616 fourcc_code('B', 'G', '4', '8')
#define DRM_FORMAT_BGR161616F fourcc_code('B', 'G', '4', 'H')
#define DRM_FORMAT_BGR323232F fourcc_code('B', 'G', '3', 'F')
#define DRM_FORMAT_ABGR32323232F fourcc_code('A', 'B', '4', 'F')
#define DRM_FORMAT_AYUV fourcc_code('A', 'Y', 'U', 'V')
#define DRM_FORMAT_UYVY fourcc_code('U', 'Y', 'V', 'Y')
#define DRM_FORMAT_VUY888 fourcc_code('V', 'U', '2', '4')
#define DRM_FORMAT_VUY101010 fourcc_code('V', 'U', '3', '0')
#define DRM_FORMAT_XYUV8888 fourcc_code('X', 'Y', 'U', 'V')
#define DRM_FORMAT_Y210 fourcc_code('Y', '2', '1', '0')
#define DRM_FORMAT_Y212 fourcc_code('Y', '2', '1', '2')
#define DRM_FORMAT_Y216 fourcc_code('Y', '2', '1', '6')
#define DRM_FORMAT_Y410 fourcc_code('Y', '4', '1', '0')
#define DRM_FORMAT_Y412 fourcc_code('Y', '4', '1', '2')
#define DRM_FORMAT_Y416 fourcc_code('Y', '4', '1', '6')
#define DRM_FORMAT_XVYU2101010 fourcc_code('X', 'V', '3', '0')
#define DRM_FORMAT_XVYU12_16161616 fourcc_code('X', 'V', '3', '6')
#define DRM_FORMAT_XVYU16161616 fourcc_code('X', 'V', '4', '8')
#define DRM_FORMAT_Y0L0 fourcc_code('Y', '0', 'L', '0')
#define DRM_FORMAT_X0L0 fourcc_code('X', '0', 'L', '0')
#define DRM_FORMAT_Y0L2 fourcc_code('Y', '0', 'L', '2')
#define DRM_FORMAT_X0L2 fourcc_code('X', '0', 'L', '2')
#define DRM_FORMAT_YUV410 fourcc_code('Y', 'U', 'V', '9')
#define DRM_FORMAT_YVU410 fourcc_code('Y', 'V', 'U', '9')
#define DRM_FORMAT_YUV411 fourcc_code('Y', 'U', '1', '1')
#define DRM_FORMAT_YVU411 fourcc_code('Y', 'V', '1', '1')
#define DRM_FORMAT_YUV420 fourcc_code('Y', 'U', '1', '2')
#define DRM_FORMAT_YVU420 fourcc_code('Y', 'V', '1', '2')
#define DRM_FORMAT_YUV422 fourcc_code('Y', 'U', '1', '6')
#define DRM_FORMAT_YVU422 fourcc_code('Y', 'V', '1', '6')
#define DRM_FORMAT_YUV444 fourcc_code('Y', 'U', '2', '4')
#define DRM_FORMAT_YVU444 fourcc_code('Y', 'V', '2', '4')
#define DRM_FORMAT_YUYV fourcc_code('Y', 'U', 'Y', 'V')
#define DRM_FORMAT_YUV420_8BIT fourcc_code('Y', 'U', '0', '8')
#define DRM_FORMAT_YUV420_10BIT fourcc_code('Y', 'U', '1', '0')

#define DRM_FORMAT_MOD_LINEAR 0ULL
#define DRM_FORMAT_MOD_INVALID ((1ULL << 56) - 1ULL)

#endif
"#,
    ),
    (
        "include/xf86drm.h",
        r#"#ifndef XF86DRM_H
#define XF86DRM_H

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

typedef uint32_t drm_magic_t;

typedef struct _drmVersion {
    int version_major;
    int version_minor;
    int version_patchlevel;
    char* name;
    int name_len;
    char* date;
    int date_len;
    char* desc;
    int desc_len;
} drmVersion, *drmVersionPtr;

#define DRM_NODE_PRIMARY 0
#define DRM_NODE_CONTROL 1
#define DRM_NODE_RENDER 2
#define DRM_NODE_MAX 3
#define DRM_CAP_SYNCOBJ_TIMELINE 0x13
#define DRM_CAP_DUMB_BUFFER 0x1
#define DRM_CLOEXEC 0x1
#define DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE (1U << 0)
#define DRM_IOCTL_SYNCOBJ_EVENTFD 0x00

typedef struct _drmDevice {
    int available_nodes;
    char* nodes[4];
} drmDevice, *drmDevicePtr;

struct drm_syncobj_eventfd {
    uint32_t handle;
    uint32_t flags;
    uint64_t point;
    int fd;
    uint32_t pad;
};

static inline char* drmGetFormatName(uint32_t format) {
    char* out = (char*)malloc(11);
    if (!out) {
        return NULL;
    }
    snprintf(out, 11, "0x%08x", format);
    return out;
}

static inline char* drmGetFormatModifierName(uint64_t modifier) {
    char* out = (char*)malloc(19);
    if (!out) {
        return NULL;
    }
    snprintf(out, 19, "0x%016llx", (unsigned long long)modifier);
    return out;
}

static inline int drmGetDevices2(uint32_t flags, drmDevicePtr devices[], int max_devices) {
    (void)flags;
    (void)devices;
    (void)max_devices;
    errno = ENODEV;
    return -ENODEV;
}

static inline int drmGetDevice(int fd, drmDevicePtr* device) {
    (void)fd;
    if (device) {
        *device = NULL;
    }
    errno = ENODEV;
    return -1;
}

static inline int drmGetDevice2(int fd, uint32_t flags, drmDevicePtr* device) {
    (void)flags;
    return drmGetDevice(fd, device);
}

static inline int drmGetDeviceFromDevId(dev_t dev_id, uint32_t flags, drmDevicePtr* device) {
    (void)dev_id;
    (void)flags;
    if (device) {
        *device = NULL;
    }
    errno = ENODEV;
    return -1;
}

static inline void drmFreeDevice(drmDevicePtr* device) {
    if (!device || !*device) {
        return;
    }
    for (size_t index = 0; index < 4; ++index) {
        free((*device)->nodes[index]);
    }
    free(*device);
    *device = NULL;
}

static inline int drmDevicesEqual(drmDevicePtr a, drmDevicePtr b) {
    if (a == b) {
        return 1;
    }
    if (!a || !b) {
        return 0;
    }

    for (size_t index = 0; index < 4; ++index) {
        const char* left = a->nodes[index];
        const char* right = b->nodes[index];
        if (!left && !right) {
            continue;
        }
        if (!left || !right || strcmp(left, right) != 0) {
            return 0;
        }
    }

    return a->available_nodes == b->available_nodes;
}

static inline int drmGetNodeTypeFromFd(int fd) {
    (void)fd;
    return DRM_NODE_RENDER;
}

static inline char* drmGetRenderDeviceNameFromFd(int fd) {
    (void)fd;
    return NULL;
}

static inline char* drmGetPrimaryDeviceNameFromFd(int fd) {
    (void)fd;
    return NULL;
}

static inline char* drmGetDeviceNameFromFd2(int fd) {
    (void)fd;
    return NULL;
}

static inline int drmGetCap(int fd, uint64_t capability, uint64_t* value) {
    (void)fd;
    (void)capability;
    if (value) {
        *value = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline drmVersionPtr drmGetVersion(int fd) {
    (void)fd;
    drmVersionPtr version = (drmVersionPtr)calloc(1, sizeof(drmVersion));
    if (!version) {
        return NULL;
    }
    version->name = strdup("macland-drm");
    version->name_len = version->name ? (int)strlen(version->name) : 0;
    return version;
}

static inline void drmFreeVersion(drmVersionPtr version) {
    if (!version) {
        return;
    }
    free(version->name);
    free(version->date);
    free(version->desc);
    free(version);
}

static inline int drmIsMaster(int fd) {
    (void)fd;
    return 0;
}

static inline int drmGetMagic(int fd, drm_magic_t* magic) {
    (void)fd;
    if (magic) {
        *magic = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmAuthMagic(int fd, drm_magic_t magic) {
    (void)fd;
    (void)magic;
    errno = ENOTSUP;
    return -1;
}

static inline int drmPrimeHandleToFD(int fd, uint32_t handle, uint32_t flags, int* prime_fd) {
    (void)fd;
    (void)handle;
    (void)flags;
    if (prime_fd) {
        *prime_fd = -1;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmPrimeFDToHandle(int fd, int prime_fd, uint32_t* handle) {
    (void)fd;
    (void)prime_fd;
    if (handle) {
        *handle = 0;
    }
    errno = ENOTSUP;
    return -1;
}

#ifndef MACLAND_DRM_CLOSE_BUFFER_HANDLE_DEFINED
#define MACLAND_DRM_CLOSE_BUFFER_HANDLE_DEFINED 1
static inline int drmCloseBufferHandle(int fd, uint32_t handle) {
    (void)fd;
    (void)handle;
    return 0;
}
#endif

static inline int drmIoctl(int fd, unsigned long request, void* arg) {
    (void)fd;
    (void)request;
    (void)arg;
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjCreate(int fd, uint32_t flags, uint32_t* handle) {
    (void)fd;
    (void)flags;
    if (handle) {
        *handle = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjDestroy(int fd, uint32_t handle) {
    (void)fd;
    (void)handle;
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjFDToHandle(int fd, int syncobj_fd, uint32_t* handle) {
    (void)fd;
    (void)syncobj_fd;
    if (handle) {
        *handle = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjHandleToFD(int fd, uint32_t handle, int* obj_fd) {
    (void)fd;
    (void)handle;
    if (obj_fd) {
        *obj_fd = -1;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjTransfer(int fd, uint32_t dst_handle, uint64_t dst_point,
        uint32_t src_handle, uint64_t src_point, uint32_t flags) {
    (void)fd;
    (void)dst_handle;
    (void)dst_point;
    (void)src_handle;
    (void)src_point;
    (void)flags;
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjExportSyncFile(int fd, uint32_t handle, int* sync_file_fd) {
    (void)fd;
    (void)handle;
    if (sync_file_fd) {
        *sync_file_fd = -1;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjImportSyncFile(int fd, uint32_t handle, int sync_file_fd) {
    (void)fd;
    (void)handle;
    (void)sync_file_fd;
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjTimelineWait(int fd, const uint32_t* handles, const uint64_t* points,
        unsigned int handle_count, int64_t timeout_nsec, unsigned flags, void* first_signaled) {
    (void)fd;
    (void)handles;
    (void)points;
    (void)handle_count;
    (void)timeout_nsec;
    (void)flags;
    (void)first_signaled;
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjTimelineSignal(int fd, const uint32_t* handles, const uint64_t* points,
        unsigned int handle_count) {
    (void)fd;
    (void)handles;
    (void)points;
    (void)handle_count;
    errno = ENOTSUP;
    return -1;
}

static inline int drmSyncobjEventfd(int fd, uint32_t handle, uint64_t point, int event_fd, uint32_t flags) {
    (void)fd;
    (void)handle;
    (void)point;
    (void)event_fd;
    (void)flags;
    errno = ENOTSUP;
    return -1;
}

static inline int drmModeCreateLease(int fd, const uint32_t* objects, int num_objects, int flags, uint32_t* lessee_id) {
    (void)fd;
    (void)objects;
    (void)num_objects;
    (void)flags;
    if (lessee_id) {
        *lessee_id = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmModeCreateDumbBuffer(int fd, uint32_t width, uint32_t height, uint32_t bpp,
        uint32_t flags, uint32_t* handle, uint32_t* pitch, uint64_t* size) {
    (void)fd;
    (void)width;
    (void)height;
    (void)bpp;
    (void)flags;
    if (handle) {
        *handle = 0;
    }
    if (pitch) {
        *pitch = 0;
    }
    if (size) {
        *size = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmModeMapDumbBuffer(int fd, uint32_t handle, uint64_t* offset) {
    (void)fd;
    (void)handle;
    if (offset) {
        *offset = 0;
    }
    errno = ENOTSUP;
    return -1;
}

static inline int drmModeDestroyDumbBuffer(int fd, uint32_t handle) {
    (void)fd;
    (void)handle;
    errno = ENOTSUP;
    return -1;
}

#endif
"#,
    ),
    (
        "include/drm_mode.h",
        r#"#ifndef DRM_MODE_H
#define DRM_MODE_H

#include <xf86drmMode.h>

#endif
"#,
    ),
    (
        "include/malloc.h",
        r#"#ifndef MALLOC_H
#define MALLOC_H

#include <stdlib.h>

#endif
"#,
    ),
    (
        "include/xf86drmMode.h",
        r#"#ifndef XF86DRMMODE_H
#define XF86DRMMODE_H

#include <errno.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

#define DRM_DISPLAY_MODE_LEN 32
#define DRM_MODE_CONTENT_TYPE_GRAPHICS 0
#define DRM_MODE_TYPE_USERDEF (1U << 5)
#define DRM_MODE_FLAG_PHSYNC (1U << 0)
#define DRM_MODE_FLAG_NHSYNC (1U << 1)
#define DRM_MODE_FLAG_PVSYNC (1U << 2)
#define DRM_MODE_FLAG_NVSYNC (1U << 3)
#define DRM_MODE_FLAG_INTERLACE (1U << 4)
#define DRM_PROP_NAME_LEN 32
#define DRM_PLANE_TYPE_OVERLAY 0
#define DRM_PLANE_TYPE_PRIMARY 1
#define DRM_PLANE_TYPE_CURSOR 2

typedef struct _drmModeModeInfo {
    uint32_t clock;
    uint16_t hdisplay;
    uint16_t hsync_start;
    uint16_t hsync_end;
    uint16_t htotal;
    uint16_t hskew;
    uint16_t vdisplay;
    uint16_t vsync_start;
    uint16_t vsync_end;
    uint16_t vtotal;
    uint16_t vscan;
    uint32_t vrefresh;
    uint32_t flags;
    uint32_t type;
    char name[DRM_DISPLAY_MODE_LEN];
} drmModeModeInfo;

typedef struct _drmModeAtomicReq {
    int cursor;
} drmModeAtomicReq;

typedef struct _drmModeFB2 {
    uint32_t fb_id;
    uint32_t width;
    uint32_t height;
    uint32_t pixel_format;
    uint64_t modifier;
    uint32_t handles[4];
    uint32_t pitches[4];
    uint32_t offsets[4];
} drmModeFB2;

typedef struct _drmModeRes {
    int count_fbs;
    int count_crtcs;
    int count_connectors;
    int count_encoders;
    uint32_t* fbs;
    uint32_t* crtcs;
    uint32_t* connectors;
    uint32_t* encoders;
    uint32_t min_width;
    uint32_t max_width;
    uint32_t min_height;
    uint32_t max_height;
} drmModeRes;

typedef struct _drmModePropertyRes {
    uint32_t prop_id;
    uint32_t flags;
    char name[DRM_PROP_NAME_LEN];
    int count_values;
    uint64_t* values;
    int count_enums;
    void* enums;
    int count_blobs;
    uint32_t* blob_ids;
} drmModePropertyRes;

typedef struct _drmModePropertyBlobRes {
    uint32_t id;
    uint32_t length;
    void* data;
} drmModePropertyBlobRes;

struct hdr_metadata_infoframe {
    uint8_t eotf;
    uint8_t metadata_type;
    struct {
        uint16_t x;
        uint16_t y;
    } display_primaries[3];
    struct {
        uint16_t x;
        uint16_t y;
    } white_point;
    uint16_t max_display_mastering_luminance;
    uint16_t min_display_mastering_luminance;
    uint16_t max_cll;
    uint16_t max_fall;
};

typedef struct hdr_output_metadata {
    uint32_t metadata_type;
    union {
        struct hdr_metadata_infoframe hdmi_metadata_type1;
    };
} hdr_output_metadata;

static inline drmModeAtomicReq* drmModeAtomicAlloc(void) {
    return (drmModeAtomicReq*)calloc(1, sizeof(drmModeAtomicReq));
}

static inline void drmModeAtomicFree(drmModeAtomicReq* req) {
    free(req);
}

static inline int drmModeAtomicAddProperty(drmModeAtomicReq* req, uint32_t object_id, uint32_t property_id, uint64_t value) {
    (void)req;
    (void)object_id;
    (void)property_id;
    (void)value;
    return 0;
}

static inline int drmModeAtomicGetCursor(drmModeAtomicReq* req) {
    return req ? req->cursor : 0;
}

static inline void drmModeAtomicSetCursor(drmModeAtomicReq* req, int cursor) {
    if (req) {
        req->cursor = cursor;
    }
}

static inline drmModeRes* drmModeGetResources(int fd) {
    (void)fd;
    return (drmModeRes*)calloc(1, sizeof(drmModeRes));
}

static inline void drmModeFreeResources(drmModeRes* resources) {
    free(resources);
}

static inline drmModeFB2* drmModeGetFB2(int fd, uint32_t fb_id) {
    drmModeFB2* info = (drmModeFB2*)calloc(1, sizeof(drmModeFB2));
    (void)fd;
    if (info) {
        info->fb_id = fb_id;
    }
    return info;
}

static inline void drmModeFreeFB2(drmModeFB2* fb) {
    free(fb);
}

#ifndef MACLAND_DRM_CLOSE_BUFFER_HANDLE_DEFINED
#define MACLAND_DRM_CLOSE_BUFFER_HANDLE_DEFINED 1
static inline int drmCloseBufferHandle(int fd, uint32_t handle) {
    (void)fd;
    (void)handle;
    return 0;
}
#endif

#endif
"#,
    ),
    (
        "include/gbm.h",
        r#"#ifndef GBM_H
#define GBM_H

#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

#define GBM_BO_USE_RENDERING (1U << 0)
#define GBM_BO_USE_SCANOUT (1U << 1)
#define GBM_BO_USE_LINEAR (1U << 2)
#define GBM_FORMAT_XRGB8888 0x34325258U
#define GBM_FORMAT_ARGB8888 0x34325241U
#define GBM_FORMAT_XBGR8888 0x34324258U
#define GBM_FORMAT_ABGR8888 0x34324241U

struct gbm_device {
    int fd;
};

struct gbm_bo {
    uint32_t width;
    uint32_t height;
    uint32_t format;
    uint64_t modifier;
};

union gbm_bo_handle {
    void* ptr;
    int32_t s32;
    uint32_t u32;
    int64_t s64;
    uint64_t u64;
};

static inline struct gbm_device* gbm_create_device(int fd) {
    struct gbm_device* device = (struct gbm_device*)calloc(1, sizeof(struct gbm_device));
    if (device) {
        device->fd = fd;
    }
    return device;
}

static inline void gbm_device_destroy(struct gbm_device* device) {
    free(device);
}

static inline int gbm_device_get_fd(struct gbm_device* device) {
    return device ? device->fd : -1;
}

static inline struct gbm_bo* gbm_bo_create_with_modifiers2(struct gbm_device* gbm, uint32_t width,
        uint32_t height, uint32_t format, const uint64_t* modifiers, const unsigned int count,
        uint32_t flags) {
    (void)gbm;
    (void)modifiers;
    (void)count;
    (void)flags;
    struct gbm_bo* bo = (struct gbm_bo*)calloc(1, sizeof(struct gbm_bo));
    if (bo) {
        bo->width = width;
        bo->height = height;
        bo->format = format;
    }
    return bo;
}

static inline void gbm_bo_destroy(struct gbm_bo* bo) {
    free(bo);
}

static inline union gbm_bo_handle gbm_bo_get_handle(struct gbm_bo* bo) {
    union gbm_bo_handle handle = {0};
    (void)bo;
    return handle;
}

static inline uint32_t gbm_bo_get_width(struct gbm_bo* bo) {
    return bo ? bo->width : 0;
}

static inline uint32_t gbm_bo_get_height(struct gbm_bo* bo) {
    return bo ? bo->height : 0;
}

static inline uint32_t gbm_bo_get_stride(struct gbm_bo* bo) {
    (void)bo;
    return 0;
}

static inline uint32_t gbm_bo_get_stride_for_plane(struct gbm_bo* bo, int plane) {
    (void)bo;
    (void)plane;
    return 0;
}

static inline uint32_t gbm_bo_get_offset(struct gbm_bo* bo, int plane) {
    (void)bo;
    (void)plane;
    return 0;
}

static inline int gbm_bo_get_plane_count(struct gbm_bo* bo) {
    (void)bo;
    return 1;
}

static inline uint32_t gbm_bo_get_format(struct gbm_bo* bo) {
    return bo ? bo->format : 0;
}

static inline uint64_t gbm_bo_get_modifier(struct gbm_bo* bo) {
    return bo ? bo->modifier : 0;
}

static inline int gbm_bo_get_fd(struct gbm_bo* bo) {
    (void)bo;
    return -1;
}

static inline int gbm_bo_get_fd_for_plane(struct gbm_bo* bo, int plane) {
    (void)bo;
    (void)plane;
    return -1;
}

#ifdef __cplusplus
}
#endif

#endif
"#,
    ),
    (
        "include/libinput.h",
        r#"#ifndef LIBINPUT_H
#define LIBINPUT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

struct udev_device;
struct libinput;
struct libinput_device {
    const char* name;
};
struct libinput_device_group;
struct libinput_config_accel;

enum libinput_config_status {
    LIBINPUT_CONFIG_STATUS_SUCCESS = 0,
    LIBINPUT_CONFIG_STATUS_UNSUPPORTED = 1,
    LIBINPUT_CONFIG_STATUS_INVALID = 2,
};

enum libinput_device_capability {
    LIBINPUT_DEVICE_CAP_KEYBOARD = 0,
    LIBINPUT_DEVICE_CAP_POINTER = 1,
    LIBINPUT_DEVICE_CAP_TOUCH = 2,
    LIBINPUT_DEVICE_CAP_TABLET_TOOL = 3,
    LIBINPUT_DEVICE_CAP_TABLET_PAD = 4,
    LIBINPUT_DEVICE_CAP_GESTURE = 5,
    LIBINPUT_DEVICE_CAP_SWITCH = 6,
};

enum libinput_config_accel_profile {
    LIBINPUT_CONFIG_ACCEL_PROFILE_NONE = 0,
    LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT = 1,
    LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE = 2,
    LIBINPUT_CONFIG_ACCEL_PROFILE_CUSTOM = 3,
};

enum libinput_config_click_method {
    LIBINPUT_CONFIG_CLICK_METHOD_NONE = 0,
    LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS = 1 << 0,
    LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER = 1 << 1,
};

enum libinput_config_clickfinger_button_map {
    LIBINPUT_CONFIG_CLICKFINGER_MAP_LRM = 0,
    LIBINPUT_CONFIG_CLICKFINGER_MAP_LMR = 1,
};

enum libinput_config_tap_state {
    LIBINPUT_CONFIG_TAP_DISABLED = 0,
    LIBINPUT_CONFIG_TAP_ENABLED = 1,
};

enum libinput_config_tap_button_map {
    LIBINPUT_CONFIG_TAP_MAP_LRM = 0,
    LIBINPUT_CONFIG_TAP_MAP_LMR = 1,
};

enum libinput_config_drag_state {
    LIBINPUT_CONFIG_DRAG_DISABLED = 0,
    LIBINPUT_CONFIG_DRAG_ENABLED = 1,
};

enum libinput_config_drag_lock_state {
    LIBINPUT_CONFIG_DRAG_LOCK_DISABLED = 0,
    LIBINPUT_CONFIG_DRAG_LOCK_ENABLED = 1,
    LIBINPUT_CONFIG_DRAG_LOCK_ENABLED_STICKY = 2,
};

enum libinput_config_dwt_state {
    LIBINPUT_CONFIG_DWT_DISABLED = 0,
    LIBINPUT_CONFIG_DWT_ENABLED = 1,
};

enum libinput_config_dwtp_state {
    LIBINPUT_CONFIG_DWTP_DISABLED = 0,
    LIBINPUT_CONFIG_DWTP_ENABLED = 1,
};

enum libinput_config_middle_emulation_state {
    LIBINPUT_CONFIG_MIDDLE_EMULATION_DISABLED = 0,
    LIBINPUT_CONFIG_MIDDLE_EMULATION_ENABLED = 1,
};

enum libinput_config_scroll_method {
    LIBINPUT_CONFIG_SCROLL_NO_SCROLL = 0,
    LIBINPUT_CONFIG_SCROLL_2FG = 1 << 0,
    LIBINPUT_CONFIG_SCROLL_EDGE = 1 << 1,
    LIBINPUT_CONFIG_SCROLL_ON_BUTTON_DOWN = 1 << 2,
};

enum libinput_config_scroll_button_lock_state {
    LIBINPUT_CONFIG_SCROLL_BUTTON_LOCK_DISABLED = 0,
    LIBINPUT_CONFIG_SCROLL_BUTTON_LOCK_ENABLED = 1,
};

enum libinput_config_send_events_mode {
    LIBINPUT_CONFIG_SEND_EVENTS_ENABLED = 0,
    LIBINPUT_CONFIG_SEND_EVENTS_DISABLED = 1 << 0,
    LIBINPUT_CONFIG_SEND_EVENTS_DISABLED_ON_EXTERNAL_MOUSE = 1 << 1,
};

enum libinput_config_3fg_drag_state {
    LIBINPUT_CONFIG_3FG_DRAG_DISABLED = 0,
    LIBINPUT_CONFIG_3FG_DRAG_ENABLED_3FG = 1,
    LIBINPUT_CONFIG_3FG_DRAG_ENABLED_4FG = 2,
};

enum libinput_accel_type {
    LIBINPUT_ACCEL_TYPE_MOTION = 0,
    LIBINPUT_ACCEL_TYPE_SCROLL = 1,
};

enum libinput_switch {
    LIBINPUT_SWITCH_LID = 0,
    LIBINPUT_SWITCH_TABLET_MODE = 1,
    LIBINPUT_SWITCH_KEYPAD_SLIDE = 2,
};

static inline const char* libinput_config_status_to_str(enum libinput_config_status status) {
    switch (status) {
    case LIBINPUT_CONFIG_STATUS_SUCCESS:
        return "success";
    case LIBINPUT_CONFIG_STATUS_UNSUPPORTED:
        return "unsupported";
    default:
        return "invalid";
    }
}

static inline int libinput_device_has_capability(struct libinput_device* device,
        enum libinput_device_capability capability) {
    (void)device;
    (void)capability;
    return 0;
}

static inline int libinput_device_get_size(struct libinput_device* device, double* width, double* height) {
    (void)device;
    if (width) {
        *width = 0.0;
    }
    if (height) {
        *height = 0.0;
    }
    return 0;
}

static inline int libinput_device_config_tap_get_finger_count(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_tap_set_enabled(
        struct libinput_device* device, enum libinput_config_tap_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_tap_state libinput_device_config_tap_get_enabled(struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_TAP_DISABLED;
}

static inline enum libinput_config_tap_state libinput_device_config_tap_get_default_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_TAP_DISABLED;
}

static inline enum libinput_config_status libinput_device_config_tap_set_button_map(
        struct libinput_device* device, enum libinput_config_tap_button_map map) {
    (void)device;
    (void)map;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_tap_button_map libinput_device_config_tap_get_button_map(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_TAP_MAP_LRM;
}

static inline enum libinput_config_tap_button_map libinput_device_config_tap_get_default_button_map(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_TAP_MAP_LRM;
}

static inline enum libinput_config_status libinput_device_config_tap_set_drag_enabled(
        struct libinput_device* device, enum libinput_config_drag_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_drag_state libinput_device_config_tap_get_drag_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DRAG_DISABLED;
}

static inline enum libinput_config_drag_state libinput_device_config_tap_get_default_drag_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DRAG_DISABLED;
}

static inline enum libinput_config_status libinput_device_config_tap_set_drag_lock_enabled(
        struct libinput_device* device, enum libinput_config_drag_lock_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_drag_lock_state libinput_device_config_tap_get_drag_lock_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DRAG_LOCK_DISABLED;
}

static inline enum libinput_config_drag_lock_state libinput_device_config_tap_get_default_drag_lock_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DRAG_LOCK_DISABLED;
}

static inline int libinput_device_config_left_handed_is_available(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_left_handed_set(
        struct libinput_device* device, int enabled) {
    (void)device;
    (void)enabled;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline int libinput_device_config_left_handed_get(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline int libinput_device_config_left_handed_get_default(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline int libinput_device_config_middle_emulation_is_available(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_middle_emulation_set_enabled(
        struct libinput_device* device, enum libinput_config_middle_emulation_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_middle_emulation_state libinput_device_config_middle_emulation_get_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_MIDDLE_EMULATION_DISABLED;
}

static inline enum libinput_config_middle_emulation_state
libinput_device_config_middle_emulation_get_default_enabled(struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_MIDDLE_EMULATION_DISABLED;
}

static inline int libinput_device_config_dwt_is_available(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_dwt_set_enabled(
        struct libinput_device* device, enum libinput_config_dwt_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_dwt_state libinput_device_config_dwt_get_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DWT_DISABLED;
}

static inline enum libinput_config_dwt_state libinput_device_config_dwt_get_default_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DWT_DISABLED;
}

static inline int libinput_device_config_dwtp_is_available(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_dwtp_set_enabled(
        struct libinput_device* device, enum libinput_config_dwtp_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_dwtp_state libinput_device_config_dwtp_get_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DWTP_DISABLED;
}

static inline enum libinput_config_dwtp_state libinput_device_config_dwtp_get_default_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_DWTP_DISABLED;
}

static inline int libinput_device_config_accel_is_available(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_accel_profile libinput_device_config_accel_get_profile(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_ACCEL_PROFILE_NONE;
}

static inline enum libinput_config_accel_profile libinput_device_config_accel_get_default_profile(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_ACCEL_PROFILE_NONE;
}

static inline enum libinput_config_status libinput_device_config_accel_set_profile(
        struct libinput_device* device, enum libinput_config_accel_profile profile) {
    (void)device;
    (void)profile;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline double libinput_device_config_accel_get_speed(struct libinput_device* device) {
    (void)device;
    return 0.0;
}

static inline double libinput_device_config_accel_get_default_speed(struct libinput_device* device) {
    (void)device;
    return 0.0;
}

static inline enum libinput_config_status libinput_device_config_accel_set_speed(
        struct libinput_device* device, double speed) {
    (void)device;
    (void)speed;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline struct libinput_config_accel* libinput_config_accel_create(enum libinput_accel_type type) {
    (void)type;
    return (struct libinput_config_accel*)0;
}

static inline enum libinput_config_status libinput_config_accel_set_points(
        struct libinput_config_accel* accel, size_t count, const double* x, const double* y) {
    (void)accel;
    (void)count;
    (void)x;
    (void)y;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_status libinput_device_config_accel_apply(
        struct libinput_device* device, struct libinput_config_accel* accel) {
    (void)device;
    (void)accel;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline void libinput_config_accel_destroy(struct libinput_config_accel* accel) {
    (void)accel;
}

static inline int libinput_device_config_rotation_is_available(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_rotation_set_angle(
        struct libinput_device* device, unsigned int angle) {
    (void)device;
    (void)angle;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline unsigned int libinput_device_config_rotation_get_angle(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline unsigned int libinput_device_config_rotation_get_default_angle(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline int libinput_device_config_scroll_has_natural_scroll(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline int libinput_device_config_scroll_get_natural_scroll_enabled(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline int libinput_device_config_scroll_get_default_natural_scroll_enabled(
        struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_scroll_set_natural_scroll_enabled(
        struct libinput_device* device, int enabled) {
    (void)device;
    (void)enabled;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline uint32_t libinput_device_config_scroll_get_methods(struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SCROLL_NO_SCROLL;
}

static inline enum libinput_config_scroll_method libinput_device_config_scroll_get_method(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SCROLL_NO_SCROLL;
}

static inline enum libinput_config_scroll_method libinput_device_config_scroll_get_default_method(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SCROLL_NO_SCROLL;
}

static inline enum libinput_config_status libinput_device_config_scroll_set_method(
        struct libinput_device* device, enum libinput_config_scroll_method method) {
    (void)device;
    (void)method;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline uint32_t libinput_device_config_scroll_get_button(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline uint32_t libinput_device_config_scroll_get_default_button(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_status libinput_device_config_scroll_set_button(
        struct libinput_device* device, uint32_t button) {
    (void)device;
    (void)button;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_scroll_button_lock_state libinput_device_config_scroll_get_button_lock(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SCROLL_BUTTON_LOCK_DISABLED;
}

static inline enum libinput_config_status libinput_device_config_scroll_set_button_lock(
        struct libinput_device* device, enum libinput_config_scroll_button_lock_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline uint32_t libinput_device_config_send_events_get_modes(struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SEND_EVENTS_ENABLED;
}

static inline enum libinput_config_send_events_mode libinput_device_config_send_events_get_mode(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SEND_EVENTS_ENABLED;
}

static inline enum libinput_config_send_events_mode libinput_device_config_send_events_get_default_mode(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_SEND_EVENTS_ENABLED;
}

static inline enum libinput_config_status libinput_device_config_send_events_set_mode(
        struct libinput_device* device, enum libinput_config_send_events_mode mode) {
    (void)device;
    (void)mode;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline int libinput_device_config_calibration_has_matrix(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline int libinput_device_config_calibration_get_matrix(struct libinput_device* device, float matrix[6]) {
    (void)device;
    if (matrix) {
        for (int i = 0; i < 6; ++i) {
            matrix[i] = 0.0f;
        }
    }
    return 0;
}

static inline int libinput_device_config_calibration_get_default_matrix(
        struct libinput_device* device, float matrix[6]) {
    return libinput_device_config_calibration_get_matrix(device, matrix);
}

static inline enum libinput_config_status libinput_device_config_calibration_set_matrix(
        struct libinput_device* device, const float matrix[6]) {
    (void)device;
    (void)matrix;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_click_method libinput_device_config_click_get_methods(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_CLICK_METHOD_NONE;
}

static inline enum libinput_config_click_method libinput_device_config_click_get_method(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_CLICK_METHOD_NONE;
}

static inline enum libinput_config_click_method libinput_device_config_click_get_default_method(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_CLICK_METHOD_NONE;
}

static inline enum libinput_config_status libinput_device_config_click_set_method(
        struct libinput_device* device, enum libinput_config_click_method method) {
    (void)device;
    (void)method;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline enum libinput_config_clickfinger_button_map libinput_device_config_click_get_clickfinger_button_map(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_CLICKFINGER_MAP_LRM;
}

static inline enum libinput_config_clickfinger_button_map
libinput_device_config_click_get_default_clickfinger_button_map(struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_CLICKFINGER_MAP_LRM;
}

static inline enum libinput_config_status libinput_device_config_click_set_clickfinger_button_map(
        struct libinput_device* device, enum libinput_config_clickfinger_button_map map) {
    (void)device;
    (void)map;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline int libinput_device_config_3fg_drag_get_finger_count(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline enum libinput_config_3fg_drag_state libinput_device_config_3fg_drag_get_default_enabled(
        struct libinput_device* device) {
    (void)device;
    return LIBINPUT_CONFIG_3FG_DRAG_DISABLED;
}

static inline enum libinput_config_status libinput_device_config_3fg_drag_set_enabled(
        struct libinput_device* device, enum libinput_config_3fg_drag_state state) {
    (void)device;
    (void)state;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

static inline const char* libinput_device_get_name(struct libinput_device* device) {
    return (device && device->name) ? device->name : "macland-libinput";
}

static inline const char* libinput_device_get_sysname(struct libinput_device* device) {
    return libinput_device_get_name(device);
}

static inline uint32_t libinput_device_get_id_vendor(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline uint32_t libinput_device_get_id_product(struct libinput_device* device) {
    (void)device;
    return 0;
}

static inline struct libinput_device_group* libinput_device_get_device_group(struct libinput_device* device) {
    (void)device;
    return (struct libinput_device_group*)0;
}

static inline struct udev_device* libinput_device_get_udev_device(struct libinput_device* device) {
    (void)device;
    return (struct udev_device*)0;
}

static inline struct libinput_device* libinput_get_device_handle(void* device) {
    return (struct libinput_device*)device;
}

static inline bool libinput_device_is_builtin(struct libinput_device* device) {
    (void)device;
    return false;
}

static inline enum libinput_config_status libinput_device_send_events(
        struct libinput_device* device, enum libinput_config_send_events_mode mode) {
    (void)device;
    (void)mode;
    return LIBINPUT_CONFIG_STATUS_SUCCESS;
}

#ifdef __cplusplus
}
#endif

#endif
"#,
    ),
    (
        "include/libevdev/libevdev.h",
        r#"#ifndef LIBEVDEV_H
#define LIBEVDEV_H

#ifdef __cplusplus
extern "C" {
#endif

static inline const char* libevdev_event_code_get_name(unsigned int type, unsigned int code) {
    (void)type;
    (void)code;
    return "macland-event";
}

static inline int libevdev_event_code_from_name(unsigned int type, const char* name) {
    (void)type;
    (void)name;
    return -1;
}

#ifdef __cplusplus
}
#endif

#endif
"#,
    ),
    (
        "include/linux/input-event-codes.h",
        r#"#ifndef LINUX_INPUT_EVENT_CODES_H
#define LINUX_INPUT_EVENT_CODES_H

#define EV_KEY 0x01

#define KEY_U 22
#define KEY_I 23
#define KEY_O 24
#define KEY_P 25
#define KEY_LEFTCTRL 29
#define KEY_LEFTSHIFT 42
#define KEY_LEFTALT 56
#define KEY_CAPSLOCK 58
#define KEY_NUMLOCK 69
#define KEY_RIGHTCTRL 97
#define KEY_RIGHTALT 100
#define KEY_RIGHTSHIFT 54
#define KEY_LEFTMETA 125
#define KEY_RIGHTMETA 126
#define KEY_MAX 0x2ff

#define BTN_LEFT 0x110
#define BTN_RIGHT 0x111
#define BTN_MIDDLE 0x112
#define BTN_SIDE 0x113
#define BTN_EXTRA 0x114
#define BTN_FORWARD 0x115
#define BTN_BACK 0x116
#define BTN_TASK 0x117
#define BTN_TOOL_PEN 0x140
#define BTN_STYLUS 0x14b
#define BTN_STYLUS2 0x14c
#define BTN_STYLUS3 0x149

#endif
"#,
    ),
    (
        "include/linux/input.h",
        r#"#ifndef MACLAND_LINUX_INPUT_H
#define MACLAND_LINUX_INPUT_H

#include <linux/input-event-codes.h>

#endif
"#,
    ),
    (
        "include/sys/eventfd.h",
        r#"#ifndef MACLAND_SYS_EVENTFD_H
#define MACLAND_SYS_EVENTFD_H

#include <libepoll-shim/sys/eventfd.h>

#endif
"#,
    ),
    (
        "include/sys/inotify.h",
        r#"#ifndef MACLAND_SYS_INOTIFY_H
#define MACLAND_SYS_INOTIFY_H

#include <errno.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define IN_MODIFY 0x00000002
#define IN_DONT_FOLLOW 0x02000000

struct inotify_event {
    int wd;
    uint32_t mask;
    uint32_t cookie;
    uint32_t len;
    char name[];
};

static inline int inotify_init(void) {
    errno = ENOSYS;
    return -1;
}

static inline int inotify_add_watch(int fd, const char* pathname, uint32_t mask) {
    (void)fd;
    (void)pathname;
    (void)mask;
    errno = ENOSYS;
    return -1;
}

static inline int inotify_rm_watch(int fd, int wd) {
    (void)fd;
    (void)wd;
    errno = ENOSYS;
    return -1;
}

#ifdef __cplusplus
}
#endif

#endif
"#,
    ),
    (
        "include/sys/signalfd.h",
        r#"#ifndef MACLAND_SYS_SIGNALFD_H
#define MACLAND_SYS_SIGNALFD_H

#include <libepoll-shim/sys/signalfd.h>

#endif
"#,
    ),
    (
        "include/sys/timerfd.h",
        r#"#ifndef MACLAND_SYS_TIMERFD_H
#define MACLAND_SYS_TIMERFD_H

#include <errno.h>
#include <stdint.h>
#include <time.h>

#if defined(__APPLE__) && !defined(MACLAND_HAVE_ITIMERSPEC)
#define MACLAND_HAVE_ITIMERSPEC 1
struct itimerspec {
    struct timespec it_interval;
    struct timespec it_value;
};
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define TFD_CLOEXEC 0x1
#define TFD_NONBLOCK 0x2
#define TFD_TIMER_ABSTIME 0x1
#define TFD_TIMER_CANCEL_ON_SET 0x2

int timerfd_create(int clockid, int flags);
int timerfd_settime(int fd, int flags, const struct itimerspec* new_value,
        struct itimerspec* old_value);
int timerfd_gettime(int fd, struct itimerspec* curr_value);

#ifdef __cplusplus
}
#endif

#endif
"#,
    ),
    (
        "lib/pkgconfig/libdrm.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
libdir=${exec_prefix}/lib
includedir=${prefix}/include

Name: libdrm
Description: macland compatibility shim for libdrm discovery
Version: 2.4.999
Cflags: -I${includedir}
Libs:
"#,
    ),
    (
        "lib/pkgconfig/gbm.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
libdir=${exec_prefix}/lib
includedir=${prefix}/include

Name: gbm
Description: macland compatibility shim for GBM discovery
Version: 25.0.0
Requires: libdrm
Cflags: -I${includedir}
Libs:
"#,
    ),
    (
        "lib/pkgconfig/libinput.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
libdir=${exec_prefix}/lib
includedir=${prefix}/include

Name: libinput
Description: macland compatibility shim for libinput discovery
Version: 1.28.99
Cflags: -I${includedir}
Libs:
"#,
    ),
    (
        "lib/pkgconfig/libudev.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
libdir=${exec_prefix}/lib
includedir=${prefix}/include

Name: libudev
Description: macland compatibility shim for libudev discovery
Version: 255.99
Cflags: -I${includedir}
Libs:
"#,
    ),
    (
        "lib/pkgconfig/libevdev.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
libdir=${exec_prefix}/lib
includedir=${prefix}/include

Name: libevdev
Description: macland compatibility shim for libevdev discovery
Version: 1.13.99
Cflags: -I${includedir}
Libs:
"#,
    ),
    (
        "include/libseat.h",
        r#"#ifndef LIBSEAT_H
#define LIBSEAT_H

#include <stdarg.h>

#ifdef __cplusplus
extern "C" {
#endif

struct libseat;

enum libseat_log_level {
    LIBSEAT_LOG_LEVEL_SILENT = 0,
    LIBSEAT_LOG_LEVEL_ERROR = 1,
    LIBSEAT_LOG_LEVEL_INFO = 2,
    LIBSEAT_LOG_LEVEL_DEBUG = 3,
    LIBSEAT_LOG_LEVEL_LAST = 4,
};

typedef void (*libseat_log_func)(enum libseat_log_level level, const char* fmt, va_list args);

struct libseat_seat_listener {
    void (*enable_seat)(struct libseat* seat, void* userdata);
    void (*disable_seat)(struct libseat* seat, void* userdata);
};

struct libseat* libseat_open_seat(const struct libseat_seat_listener* listener, void* userdata);
int libseat_disable_seat(struct libseat* seat);
int libseat_close_seat(struct libseat* seat);
int libseat_open_device(struct libseat* seat, const char* path, int* fd);
int libseat_close_device(struct libseat* seat, int device_id);
const char* libseat_seat_name(struct libseat* seat);
int libseat_switch_session(struct libseat* seat, int session);
int libseat_get_fd(struct libseat* seat);
int libseat_dispatch(struct libseat* seat, int timeout);
void libseat_set_log_level(enum libseat_log_level level);

#ifdef __cplusplus
}
#endif

#endif
"#,
    ),
    (
        "lib/pkgconfig/libseat.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
libdir=${exec_prefix}/lib
includedir=${prefix}/include

Name: libseat
Description: macland compatibility shim for libseat discovery
Version: 0.2.0
Cflags: -I${includedir}
Libs: -L${libdir} -lseat
"#,
    ),
    (
        "share/hwdata/pnp.ids",
        r#"AAA	Fake Vendor
ACR	Acer
APP	Apple
DEL	Dell
SAM	Samsung
"#,
    ),
    (
        "lib/pkgconfig/hwdata.pc",
        r#"prefix=${pcfiledir}/../..
exec_prefix=${prefix}
datarootdir=${prefix}/share
pkgdatadir=${datarootdir}/hwdata

Name: hwdata
Description: macland compatibility shim for hwdata lookup
Version: 0.0.1
"#,
    ),
];

const STUB_LIBRARIES: &[(&str, &str)] = &[
    (
        "librt.a",
        r#"#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>

#if defined(__APPLE__) && !defined(MACLAND_HAVE_ITIMERSPEC)
#define MACLAND_HAVE_ITIMERSPEC 1
struct itimerspec {
    struct timespec it_interval;
    struct timespec it_value;
};
#endif

int eventfd(unsigned int initval, int flags) {
    (void)initval;
    (void)flags;
    errno = ENOSYS;
    return -1;
}

int epoll_shim_close(int fd) {
    return close(fd);
}

ssize_t epoll_shim_read(int fd, void* buf, size_t count) {
    return read(fd, buf, count);
}

int epoll_shim_poll(struct pollfd* fds, nfds_t nfds, int timeout) {
    return poll(fds, nfds, timeout);
}

int signalfd(int fd, const sigset_t* mask, int flags) {
    (void)fd;
    (void)mask;
    (void)flags;
    errno = ENOSYS;
    return -1;
}

int timerfd_create(int clockid, int flags) {
    (void)clockid;
    (void)flags;
    errno = ENOSYS;
    return -1;
}

int timerfd_settime(int fd, int flags, const struct itimerspec* new_value,
        struct itimerspec* old_value) {
    (void)fd;
    (void)flags;
    (void)new_value;
    (void)old_value;
    errno = ENOSYS;
    return -1;
}

int timerfd_gettime(int fd, struct itimerspec* curr_value) {
    (void)fd;
    if (curr_value) {
        curr_value->it_interval.tv_sec = 0;
        curr_value->it_interval.tv_nsec = 0;
        curr_value->it_value.tv_sec = 0;
        curr_value->it_value.tv_nsec = 0;
    }
    errno = ENOSYS;
    return -1;
}

int macland_librt_stub(void) {
    return 0;
}
"#,
    ),
    (
        "libseat.a",
        r#"#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "libseat.h"

struct libseat {
    const struct libseat_seat_listener* listener;
    void* userdata;
    int fd_pair[2];
};

struct libseat* libseat_open_seat(const struct libseat_seat_listener* listener, void* userdata) {
    struct libseat* seat = calloc(1, sizeof(struct libseat));
    if (!seat) {
        return NULL;
    }
    seat->listener = listener;
    seat->userdata = userdata;
    if (pipe(seat->fd_pair) != 0) {
        seat->fd_pair[0] = -1;
        seat->fd_pair[1] = -1;
    }
    if (listener && listener->enable_seat) {
        listener->enable_seat(seat, userdata);
    }
    return seat;
}

int libseat_disable_seat(struct libseat* seat) {
    if (seat && seat->listener && seat->listener->disable_seat) {
        seat->listener->disable_seat(seat, seat->userdata);
    }
    return 0;
}

int libseat_close_seat(struct libseat* seat) {
    if (!seat) {
        return 0;
    }
    if (seat->fd_pair[0] >= 0) {
        close(seat->fd_pair[0]);
    }
    if (seat->fd_pair[1] >= 0) {
        close(seat->fd_pair[1]);
    }
    free(seat);
    return 0;
}

int libseat_open_device(struct libseat* seat, const char* path, int* fd) {
    (void)seat;
    (void)path;
    if (fd) {
        *fd = -1;
    }
    errno = ENOSYS;
    return -1;
}

int libseat_close_device(struct libseat* seat, int device_id) {
    (void)seat;
    (void)device_id;
    return 0;
}

const char* libseat_seat_name(struct libseat* seat) {
    (void)seat;
    return "macland-seat";
}

int libseat_switch_session(struct libseat* seat, int session) {
    (void)seat;
    (void)session;
    errno = ENOSYS;
    return -1;
}

int libseat_get_fd(struct libseat* seat) {
    if (!seat) {
        errno = EINVAL;
        return -1;
    }
    return seat->fd_pair[0];
}

int libseat_dispatch(struct libseat* seat, int timeout) {
    (void)seat;
    (void)timeout;
    return 0;
}

void libseat_set_log_level(enum libseat_log_level level) {
    (void)level;
}
"#,
    ),
];

pub const DEPENDENCIES: &[&str] = &[
    "libdrm", "gbm", "libinput", "libudev", "libseat", "libevdev",
];

pub fn install_workspace_shims(workspace_root: &Path) -> Result<PathBuf, String> {
    let sysroot = workspace_root.join(".macland").join("sysroot");
    for (relative, contents) in FILES {
        let path = sysroot.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&path, contents).map_err(|err| err.to_string())?;
    }
    install_stub_libraries(&sysroot)?;
    patch_vendor_headers(&sysroot)?;
    Ok(sysroot)
}

fn patch_vendor_headers(sysroot: &Path) -> Result<(), String> {
    let aquamarine_backend = sysroot.join("include/aquamarine/backend/Backend.hpp");
    if aquamarine_backend.exists() {
        let contents = fs::read_to_string(&aquamarine_backend).map_err(|err| err.to_string())?;
        if !contents.contains("#include \"Session.hpp\"") {
            let updated = if contents.contains("#include \"Misc.hpp\"") {
                contents.replace(
                    "#include \"Misc.hpp\"",
                    "#include \"Misc.hpp\"\n#include \"Session.hpp\"",
                )
            } else {
                format!("#include \"Session.hpp\"\n{contents}")
            };
            fs::write(&aquamarine_backend, updated).map_err(|err| err.to_string())?;
        }
    }

    Ok(())
}

fn install_stub_libraries(sysroot: &Path) -> Result<(), String> {
    let staging = sysroot.join(".stubs");
    fs::create_dir_all(&staging).map_err(|err| err.to_string())?;

    for (library_name, source) in STUB_LIBRARIES {
        let stem = library_name
            .strip_prefix("lib")
            .and_then(|name| name.strip_suffix(".a"))
            .unwrap_or(library_name);
        let source_path = staging.join(format!("{stem}.c"));
        let object_path = staging.join(format!("{stem}.o"));
        let library_path = sysroot.join("lib").join(library_name);

        fs::write(&source_path, source).map_err(|err| err.to_string())?;
        let compile_status = Command::new("cc")
            .args([
                "-c",
                source_path.to_string_lossy().as_ref(),
                "-I",
                sysroot.join("include").to_string_lossy().as_ref(),
                "-o",
                object_path.to_string_lossy().as_ref(),
            ])
            .status()
            .map_err(|err| err.to_string())?;
        if !compile_status.success() {
            return Err(format!(
                "failed to compile stub library source for {library_name}"
            ));
        }

        let archive_status = Command::new("libtool")
            .args([
                "-static",
                "-o",
                library_path.to_string_lossy().as_ref(),
                object_path.to_string_lossy().as_ref(),
            ])
            .status()
            .map_err(|err| err.to_string())?;
        if !archive_status.success() {
            return Err(format!("failed to archive stub library {library_name}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DEPENDENCIES, install_workspace_shims};
    use std::fs;

    #[test]
    fn installs_workspace_shims() {
        let temp = std::env::temp_dir().join(format!("macland-shims-{}", std::process::id()));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(&temp).unwrap();

        let sysroot = install_workspace_shims(&temp).unwrap();
        for dependency in DEPENDENCIES {
            assert!(
                sysroot
                    .join("lib/pkgconfig")
                    .join(format!("{dependency}.pc"))
                    .exists()
            );
        }
        assert!(sysroot.join("include/drm_fourcc.h").exists());
        assert!(sysroot.join("include/gbm.h").exists());
        assert!(sysroot.join("include/libinput.h").exists());
        assert!(sysroot.join("include/libseat.h").exists());
        assert!(sysroot.join("include/libevdev/libevdev.h").exists());
        assert!(sysroot.join("include/linux/input-event-codes.h").exists());
        assert!(sysroot.join("include/linux/input.h").exists());
        assert!(sysroot.join("include/sys/inotify.h").exists());
        assert!(sysroot.join("include/sys/timerfd.h").exists());
        assert!(sysroot.join("lib/librt.a").exists());
        assert!(sysroot.join("lib/libseat.a").exists());
        let xf86drm = fs::read_to_string(sysroot.join("include/xf86drm.h")).unwrap();
        assert!(xf86drm.contains("drmGetVersion"));
        assert!(xf86drm.contains("drmFreeVersion"));
        assert!(xf86drm.contains("drmDevicesEqual"));
        let input_codes =
            fs::read_to_string(sysroot.join("include/linux/input-event-codes.h")).unwrap();
        assert!(input_codes.contains("#define EV_KEY 0x01"));
        let rt_stub = fs::read_to_string(sysroot.join(".stubs/rt.c")).unwrap();
        assert!(rt_stub.contains("int eventfd("));
        assert!(rt_stub.contains("int signalfd("));
        assert!(rt_stub.contains("int timerfd_settime("));

        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn patches_aquamarine_backend_header() {
        let temp =
            std::env::temp_dir().join(format!("macland-aquamarine-header-{}", std::process::id()));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }

        let backend_header = temp.join(".macland/sysroot/include/aquamarine/backend/Backend.hpp");
        fs::create_dir_all(backend_header.parent().unwrap()).unwrap();
        fs::write(
            &backend_header,
            "#pragma once\n#include \"Misc.hpp\"\nnamespace Aquamarine { class CSession; }\n",
        )
        .unwrap();

        let sysroot = install_workspace_shims(&temp).unwrap();
        let patched =
            fs::read_to_string(sysroot.join("include/aquamarine/backend/Backend.hpp")).unwrap();
        assert!(patched.contains("#include \"Session.hpp\""));

        fs::remove_dir_all(&temp).unwrap();
    }
}
