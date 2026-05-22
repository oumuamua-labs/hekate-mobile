// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

/// `request()` called from arbitrary thread
/// on `Task` cancel. Must be thread-safe and
/// idempotent. Polled in Rust, not Swift.
public protocol CancellableToken: AnyObject, Sendable {
    func request()
}