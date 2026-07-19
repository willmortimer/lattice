#pragma once

/**
 * Shared C types for LatticeAudioBridge.
 * Function implementations are Swift `@_cdecl` exports (see BridgeExports.swift).
 * The full ABI header for Rust lives at crates/lattice-audio-macos/include/.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LATTICE_AUDIO_BRIDGE_ABI_VERSION 1u

typedef uint64_t lattice_audio_capture_t;

enum {
    LATTICE_AUDIO_OK = 0,
    LATTICE_AUDIO_ERR_INVALID_ARG = -1,
    LATTICE_AUDIO_ERR_NOT_ARMED = -2,
    LATTICE_AUDIO_ERR_ALREADY_RUNNING = -3,
    LATTICE_AUDIO_ERR_PERMISSION = -4,
    LATTICE_AUDIO_ERR_DEVICE = -5,
    LATTICE_AUDIO_ERR_INTERNAL = -6,
    LATTICE_AUDIO_ERR_UNSUPPORTED = -7,
    LATTICE_AUDIO_ERR_NOT_RUNNING = -8
};

typedef enum lattice_audio_event_kind {
    LATTICE_AUDIO_EVENT_STARTED = 1,
    LATTICE_AUDIO_EVENT_FRAME = 2,
    LATTICE_AUDIO_EVENT_GAP = 3,
    LATTICE_AUDIO_EVENT_STOPPED = 4,
    LATTICE_AUDIO_EVENT_ERROR = 5
} lattice_audio_event_kind_t;

typedef struct lattice_audio_frame {
    uint64_t sequence;
    uint64_t captured_at_ns;
    uint32_t frame_count;
    const float *samples;
    float peak_abs;
    float rms;
    uint8_t clipped;
    uint8_t _pad[3];
} lattice_audio_frame_t;

typedef struct lattice_audio_gap {
    uint64_t from_sequence;
    uint64_t to_sequence;
    uint64_t captured_at_ns;
} lattice_audio_gap_t;

typedef struct lattice_audio_event {
    lattice_audio_event_kind_t kind;
    uint64_t captured_at_ns;
    lattice_audio_frame_t frame;
    lattice_audio_gap_t gap;
    int32_t error_code;
    const char *error_message;
    uint32_t error_message_len;
} lattice_audio_event_t;

typedef void (*lattice_audio_event_callback)(
    const lattice_audio_event_t *event,
    void *context
);

#ifdef __cplusplus
}
#endif
