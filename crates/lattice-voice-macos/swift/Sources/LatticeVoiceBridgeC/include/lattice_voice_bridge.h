#pragma once

/**
 * Shared C types for LatticeVoiceBridge.
 * Function implementations are Swift `@_cdecl` exports (see BridgeExports.swift).
 * The full ABI header for Rust lives at crates/lattice-voice-macos/include/.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

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
    LATTICE_VOICE_EVENT_SPEECH_STARTED = 5,
    /** error_code: 0=silence, 1=max_utterance, 2=provider_eou */
    LATTICE_VOICE_EVENT_ENDPOINT = 6
} lattice_voice_event_kind_t;

typedef struct lattice_voice_event {
    lattice_voice_event_kind_t kind;
    const char *text;
    uint32_t text_len;
    uint32_t stable_prefix_bytes;
    int32_t error_code;
} lattice_voice_event_t;

typedef void (*lattice_voice_event_callback)(
    const lattice_voice_event_t *event,
    void *context
);

#ifdef __cplusplus
}
#endif
