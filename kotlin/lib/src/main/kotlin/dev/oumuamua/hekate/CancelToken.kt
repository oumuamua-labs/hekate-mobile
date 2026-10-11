// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate

/** `request()` called from arbitrary thread
 *  on `Job` cancel. Must be thread-safe and
 *  idempotent. Polled in Rust, not Kotlin. */
public interface CancelToken {
    public fun request()
}