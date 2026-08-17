#pragma once

/**
 * Shared C types for LatticeCaptureBridge.
 * Function implementations are Swift `@_cdecl` exports (see BridgeExports.swift).
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

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

uint32_t lattice_capture_bridge_abi_version(void);

int32_t lattice_capture_enumerate_displays(
    lattice_capture_display_info_t *out,
    uint32_t out_capacity,
    uint32_t *out_count
);

int32_t lattice_capture_enumerate_windows(
    lattice_capture_window_info_t *out,
    uint32_t out_capacity,
    uint32_t *out_count
);

int32_t lattice_capture_capture_display(
    uint32_t display_id,
    lattice_capture_image_out_t *out_image
);

int32_t lattice_capture_capture_window(
    uint64_t window_id,
    lattice_capture_image_out_t *out_image
);

int32_t lattice_capture_capture_region(
    uint32_t display_id,
    const lattice_capture_region_t *region,
    lattice_capture_image_out_t *out_image
);

int32_t lattice_capture_select_interactive_region(
    uint32_t *out_display_id,
    lattice_capture_region_t *out_region
);

int32_t lattice_capture_select_interactive_window(
    uint64_t *out_window_id
);

int32_t lattice_capture_capture_interactive_region(
    lattice_capture_image_out_t *out_image
);

void lattice_capture_image_release(lattice_capture_image_out_t *image);

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

int32_t lattice_capture_permission_status(
    lattice_capture_permission_status_t *out_status
);

int32_t lattice_capture_permission_request(
    lattice_capture_permission_status_t *out_status
);

int32_t lattice_capture_permission_open_settings(void);

#ifdef __cplusplus
}
#endif
