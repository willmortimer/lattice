#ifndef LATTICE_APPLE_SIGNIN_BRIDGE_H
#define LATTICE_APPLE_SIGNIN_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

uint32_t lattice_apple_signin_bridge_abi_version(void);

/**
 * Present Sign in with Apple and return the identity token (UTF-8 C string).
 * `nonce` may be NULL. On success, *out_token is allocated; free with
 * lattice_apple_signin_string_free. On failure, *out_error may be set.
 * Returns 0 on success.
 */
int32_t lattice_apple_signin_request(
    const char *nonce,
    char **out_token,
    char **out_error
);

void lattice_apple_signin_string_free(char *ptr);

#ifdef __cplusplus
}
#endif

#endif /* LATTICE_APPLE_SIGNIN_BRIDGE_H */
