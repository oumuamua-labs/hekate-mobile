// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

/// `request()` called from arbitrary thread
/// on `Task` cancel. Must be thread-safe and
/// idempotent. Polled in Rust, not Swift.
public protocol CancellableToken: AnyObject, Sendable {
    func request()
}