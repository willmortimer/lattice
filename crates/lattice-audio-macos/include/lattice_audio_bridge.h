#pragma once

/**
 * LatticeAudioBridge C ABI (version 1).
 *
 * Opaque-handle surface between Rust and Swift AVAudioEngine capture.
 * Audio on the wire: Float32 little-endian, 16 kHz, mono.
 *
 * Frame sample pointers in callbacks are valid only for the duration of the
 * callback; the Rust side must copy before returning.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Must match `LATTICE_AUDIO_BRIDGE_ABI_VERSION` in Rust and Swift. */
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
    /** Float32 LE mono @ 16 kHz; valid only during the callback. */
    const float *samples;
    /** Peak absolute sample; NAN when diagnostics disabled. */
    float peak_abs;
    /** RMS level; NAN when diagnostics disabled. */
    float rms;
    /** Non-zero when any sample clipped. */
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
    /** UTF-8 error message; NULL when empty. Valid only during callback. */
    const char *error_message;
    uint32_t error_message_len;
} lattice_audio_event_t;

typedef void (*lattice_audio_event_callback)(
    const lattice_audio_event_t *event,
    void *context
);

/**
 * Returns `LATTICE_AUDIO_BRIDGE_ABI_VERSION`.
 * Rust must reject mismatched versions before creating captures.
 */
uint32_t lattice_audio_bridge_abi_version(void);

/**
 * Create a capture handle.
 * `pre_roll_ms` of 0 selects the default (300 ms).
 * `enable_diagnostics` non-zero fills peak/rms/clipped on frames.
 */
int32_t lattice_audio_capture_create(
    uint32_t pre_roll_ms,
    uint8_t enable_diagnostics,
    lattice_audio_capture_t *out_capture
);

/**
 * Arm the mic: fill the pre-roll ring without emitting stream frames.
 * Requests microphone permission if needed.
 */
int32_t lattice_audio_capture_arm(lattice_audio_capture_t capture);

/**
 * Start streaming. Emits STARTED, flushes pre-roll as FRAME events, then live
 * FRAMEs. `callback` may run on the audio realtime thread — keep it short.
 */
int32_t lattice_audio_capture_start(
    lattice_audio_capture_t capture,
    lattice_audio_event_callback callback,
    void *context
);

/** Stop capture and emit STOPPED. */
int32_t lattice_audio_capture_stop(lattice_audio_capture_t capture);

/** Destroy a capture handle. Stop first if running. */
void lattice_audio_capture_destroy(lattice_audio_capture_t capture);

#ifdef __cplusplus
}
#endif
