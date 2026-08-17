#pragma once

/**
 * LatticeCaptureBridge C ABI (version 1).
 *
 * Opaque-handle surface between Rust and Swift ScreenCaptureKit capture.
 * Screenshot output is PNG bytes allocated by the bridge; Rust must copy
 * then call `lattice_capture_image_release`.
 *
 * Never shells to `/usr/sbin/screencapture`.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Must match `LATTICE_CAPTURE_BRIDGE_ABI_VERSION` in Rust and Swift. */
#define LATTICE_CAPTURE_BRIDGE_ABI_VERSION 1u

enum {
    LATTICE_CAPTURE_OK = 0,
    LATTICE_CAPTURE_ERR_INVALID_ARG = -1,
    LATTICE_CAPTURE_ERR_CANCELLED = -2,
    LATTICE_CAPTURE_ERR_PERMISSION = -3,
    LATTICE_CAPTURE_ERR_NOT_FOUND = -4,
    LATTICE_CAPTURE_ERR_INTERNAL = -5,
    LATTICE_CAPTURE_ERR_UNSUPPORTED = -6,
    LATTICE_CAPTURE_ERR_NOT_IMPLEMENTED = -7
};

typedef struct lattice_capture_display_info {
    uint32_t display_id;
    uint32_t width;
    uint32_t height;
} lattice_capture_display_info_t;

/** UTF-8 window title capacity (NUL-terminated). */
#define LATTICE_CAPTURE_WINDOW_TITLE_MAX 256u

typedef struct lattice_capture_window_info {
    uint64_t window_id;
    uint32_t width;
    uint32_t height;
    uint8_t title[LATTICE_CAPTURE_WINDOW_TITLE_MAX];
} lattice_capture_window_info_t;

typedef struct lattice_capture_image_out {
    uint32_t width;
    uint32_t height;
    uint8_t *png_bytes;
    uint32_t png_len;
} lattice_capture_image_out_t;

typedef struct lattice_capture_region {
    int32_t x;
    int32_t y;
    uint32_t width;
    uint32_t height;
} lattice_capture_region_t;

/**
 * Returns `LATTICE_CAPTURE_BRIDGE_ABI_VERSION`.
 * Rust must reject mismatched versions before calling other entry points.
 */
uint32_t lattice_capture_bridge_abi_version(void);

/**
 * Enumerate on-screen displays. Writes up to `out_capacity` rows into `out`
 * and sets `*out_count` to the number written.
 */
int32_t lattice_capture_enumerate_displays(
    lattice_capture_display_info_t *out,
    uint32_t out_capacity,
    uint32_t *out_count
);

/**
 * Enumerate on-screen titled windows. Writes up to `out_capacity` rows into
 * `out` and sets `*out_count` to the number written. `window_id` is
 * `CGWindowID` widened to 64-bit. Titles are UTF-8, NUL-terminated.
 */
int32_t lattice_capture_enumerate_windows(
    lattice_capture_window_info_t *out,
    uint32_t out_capacity,
    uint32_t *out_count
);

/**
 * Capture a full display as PNG via ScreenCaptureKit.
 * `display_id` is `CGDirectDisplayID`.
 */
int32_t lattice_capture_capture_display(
    uint32_t display_id,
    lattice_capture_image_out_t *out_image
);

/**
 * Capture a specific on-screen window as PNG via ScreenCaptureKit.
 * `window_id` is `CGWindowID` widened to 64-bit.
 */
int32_t lattice_capture_capture_window(
    uint64_t window_id,
    lattice_capture_image_out_t *out_image
);

/**
 * Capture a fixed screen region as PNG via ScreenCaptureKit.
 */
int32_t lattice_capture_capture_region(
    uint32_t display_id,
    const lattice_capture_region_t *region,
    lattice_capture_image_out_t *out_image
);

/**
 * Present an interactive AppKit region selector overlay.
 * On success writes display id + display-local region (top-left origin, points).
 * Escape / empty drag returns `LATTICE_CAPTURE_ERR_CANCELLED`.
 */
int32_t lattice_capture_select_interactive_region(
    uint32_t *out_display_id,
    lattice_capture_region_t *out_region
);

/**
 * Present an interactive AppKit overlay to click-target an on-screen window.
 * On success writes `CGWindowID` (widened) into `*out_window_id`.
 * Escape returns `LATTICE_CAPTURE_ERR_CANCELLED`.
 */
int32_t lattice_capture_select_interactive_window(
    uint64_t *out_window_id
);

/**
 * Select an interactive region then capture it as PNG via ScreenCaptureKit.
 * Prefer composing `lattice_capture_select_interactive_region` +
 * `lattice_capture_capture_region` from Rust when possible.
 */
int32_t lattice_capture_capture_interactive_region(
    lattice_capture_image_out_t *out_image
);

/** Release PNG bytes allocated by capture entry points. */
void lattice_capture_image_release(lattice_capture_image_out_t *image);

/** Screen recording permission state values for `lattice_capture_permission_status_t`. */
enum {
    LATTICE_CAPTURE_PERM_UNSUPPORTED = 0,
    LATTICE_CAPTURE_PERM_NOT_DETERMINED = 1,
    LATTICE_CAPTURE_PERM_AUTHORIZED = 2,
    LATTICE_CAPTURE_PERM_DENIED = 3,
    LATTICE_CAPTURE_PERM_RESTRICTED = 4
};

typedef struct lattice_capture_permission_status {
    uint32_t state;
} lattice_capture_permission_status_t;

/**
 * Read current screen recording permission without prompting.
 */
int32_t lattice_capture_permission_status(
    lattice_capture_permission_status_t *out_status
);

/**
 * Request screen recording permission (may show the system prompt).
 */
int32_t lattice_capture_permission_request(
    lattice_capture_permission_status_t *out_status
);

/** Open System Settings → Privacy & Security → Screen Recording. */
int32_t lattice_capture_permission_open_settings(void);

#ifdef __cplusplus
}
#endif
