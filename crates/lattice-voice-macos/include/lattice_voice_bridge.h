#pragma once

/**
 * LatticeVoiceBridge C ABI (version 1).
 *
 * Opaque-handle surface between Rust and the Swift FluidAudio Unified path.
 * Audio on the wire: Float32 little-endian, 16 kHz, mono.
 *
 * Transcript strings in callbacks are valid only for the duration of the
 * callback; the Rust side must copy before returning.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Must match `LATTICE_VOICE_BRIDGE_ABI_VERSION` in Rust and Swift. */
#define LATTICE_VOICE_BRIDGE_ABI_VERSION 1u

typedef uint64_t lattice_voice_engine_t;
typedef uint64_t lattice_voice_session_t;

enum {
    LATTICE_VOICE_OK = 0,
    LATTICE_VOICE_ERR_INVALID_ARG = -1,
    LATTICE_VOICE_ERR_NOT_PREPARED = -2,
    LATTICE_VOICE_ERR_ALREADY_PREPARED = -3,
    LATTICE_VOICE_ERR_SESSION = -4,
    LATTICE_VOICE_ERR_CANCELLED = -5,
    LATTICE_VOICE_ERR_INTERNAL = -6,
    LATTICE_VOICE_ERR_UNSUPPORTED = -7,
    LATTICE_VOICE_ERR_NOT_FOUND = -8
};

typedef enum lattice_voice_event_kind {
    LATTICE_VOICE_EVENT_PARTIAL = 1,
    LATTICE_VOICE_EVENT_STABLE = 2,
    LATTICE_VOICE_EVENT_FINAL = 3,
    LATTICE_VOICE_EVENT_ERROR = 4,
    /** Energy / EOU speech onset. */
    LATTICE_VOICE_EVENT_SPEECH_STARTED = 5,
    /**
     * Utterance endpoint (silence debounce, max length, or provider EOU).
     * `error_code` carries reason: 0=silence, 1=max_utterance, 2=provider_eou.
     */
    LATTICE_VOICE_EVENT_ENDPOINT = 6
} lattice_voice_event_kind_t;

typedef struct lattice_voice_event {
    lattice_voice_event_kind_t kind;
    /** UTF-8 transcript or error message; NULL when empty. Valid only during callback. */
    const char *text;
    uint32_t text_len;
    /** Stable UTF-8 byte prefix for provisional UI (0 when unknown). */
    uint32_t stable_prefix_bytes;
    /** Non-zero only for LATTICE_VOICE_EVENT_ERROR. */
    int32_t error_code;
} lattice_voice_event_t;

typedef void (*lattice_voice_event_callback)(
    const lattice_voice_event_t *event,
    void *context
);

/**
 * Returns `LATTICE_VOICE_BRIDGE_ABI_VERSION`.
 * Rust must reject mismatched versions before creating sessions.
 */
uint32_t lattice_voice_bridge_abi_version(void);

/**
 * Create an engine. `model_cache_dir` is a UTF-8 path to a Models directory
 * (FluidAudio download root). Pass NULL to use
 * `$LATTICE_VOICE_MODEL_CACHE` or a default under Application Support.
 * On success writes a non-zero opaque handle to `out_engine`.
 */
int32_t lattice_voice_engine_create(
    const char *model_cache_dir,
    lattice_voice_engine_t *out_engine
);

/**
 * Load the Unified streaming checkpoint (`parakeet-unified-320ms`).
 * Blocks until models are ready or an error occurs.
 */
int32_t lattice_voice_engine_prepare(lattice_voice_engine_t engine);

/** Destroy an engine. Destroy all sessions first. */
void lattice_voice_engine_destroy(lattice_voice_engine_t engine);

/**
 * Start a session on a prepared engine.
 * Callbacks may arrive on background threads; never from the Tokio executor.
 */
int32_t lattice_voice_session_start(
    lattice_voice_engine_t engine,
    lattice_voice_event_callback callback,
    void *context,
    lattice_voice_session_t *out_session
);

/**
 * Push Float32 LE mono @ 16 kHz samples.
 * The bridge copies immediately; `samples` need only remain valid for the call.
 */
int32_t lattice_voice_session_push_audio(
    lattice_voice_session_t session,
    const float *samples,
    size_t sample_count
);

/**
 * Run Unified `finish()` and emit one authoritative final event.
 * Blocks until the final is emitted or an error occurs.
 */
int32_t lattice_voice_session_finish_utterance(lattice_voice_session_t session);

/**
 * Cancel the session. After success, late callbacks are dropped.
 */
int32_t lattice_voice_session_cancel(lattice_voice_session_t session);

/** Destroy a session handle. */
void lattice_voice_session_destroy(lattice_voice_session_t session);

#ifdef __cplusplus
}
#endif
