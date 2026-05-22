// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

/// UniFFI-generated. Do not implement by hand.
public protocol HekateProver {
    associatedtype Inputs: Sendable
    associatedtype Output: Sendable
    associatedtype Token: CancellableToken

    static func prove(inputs: Inputs, cancel: Token) throws -> Output
    static func makeCancelToken() -> Token
}

public extension HekateProver {
    /// `priority: .background` for batch/off-screen
    /// work, `.userInitiated` risks the iOS
    /// background-CPU killer. Cancellation
    /// flows via `token.request()`.
    static func prove(
        inputs: Inputs,
        priority: TaskPriority? = nil
    ) async throws -> Output {
        let token = makeCancelToken()

        return try await withTaskCancellationHandler {
            try await Task.detached(priority: priority) {
                try Self.prove(inputs: inputs, cancel: token)
            }.value
        } onCancel: {
            token.request()
        }
    }
}