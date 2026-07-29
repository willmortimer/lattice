#ifndef LATTICE_APPROVAL_BRIDGE_H
#define LATTICE_APPROVAL_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

uint32_t lattice_approval_bridge_abi_version(void);

/** Create or restore the Secure Enclave approval signer. 0 = ok. */
int32_t lattice_approval_load_or_create(void);

void lattice_approval_shutdown(void);

/** Static C string: "secure-enclave". */
const char *lattice_approval_backend(void);

/** Caller must free with lattice_approval_string_free. */
int32_t lattice_approval_device_id(char **out);
int32_t lattice_approval_key_id(char **out);

/** Caller must free signature with lattice_approval_buffer_free. */
int32_t lattice_approval_sign(
    const uint8_t *payload,
    size_t payload_len,
    uint8_t **out_sig,
    size_t *out_len
);

void lattice_approval_string_free(char *ptr);
void lattice_approval_buffer_free(uint8_t *ptr, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* LATTICE_APPROVAL_BRIDGE_H */
