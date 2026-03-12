use std::fs;
use std::path::{Path, PathBuf};

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

#define DRM_FORMAT_MOD_LINEAR 0ULL
#define DRM_FORMAT_MOD_INVALID ((1ULL << 56) - 1ULL)

#endif
"#,
    ),
    (
        "include/xf86drm.h",
        r#"#ifndef XF86DRM_H
#define XF86DRM_H

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef uint32_t drm_magic_t;

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

#endif
"#,
    ),
    (
        "include/xf86drmMode.h",
        r#"#ifndef XF86DRMMODE_H
#define XF86DRMMODE_H

#include <stdint.h>

#define DRM_DISPLAY_MODE_LEN 32
#define DRM_MODE_CONTENT_TYPE_GRAPHICS 0

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

#include <libepoll-shim/sys/timerfd.h>

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
];

pub const DEPENDENCIES: &[&str] = &["libdrm", "gbm", "libinput", "libudev"];

pub fn install_workspace_shims(workspace_root: &Path) -> Result<PathBuf, String> {
    let sysroot = workspace_root.join(".macland").join("sysroot");
    for (relative, contents) in FILES {
        let path = sysroot.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&path, contents).map_err(|err| err.to_string())?;
    }
    Ok(sysroot)
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
            assert!(sysroot
                .join("lib/pkgconfig")
                .join(format!("{dependency}.pc"))
                .exists());
        }
        assert!(sysroot.join("include/drm_fourcc.h").exists());

        fs::remove_dir_all(&temp).unwrap();
    }
}
