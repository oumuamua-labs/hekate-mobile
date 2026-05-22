// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

package dev.oumuamua.hekate

/** `request()` called from arbitrary thread
 *  on `Job` cancel. Must be thread-safe and
 *  idempotent. Polled in Rust, not Kotlin. */
public interface CancelToken {
    public fun request()
}