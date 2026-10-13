// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation
import Hekate

struct Sample: Sendable {
    let programId: String
    let proofBytes: Int
    let witness: TimeInterval
    let prove: TimeInterval
    let wholeCall: TimeInterval
    let verify: TimeInterval
}

struct Measurement: Sendable {
    let keyStorage: String
    let samples: [Sample]
}

enum Benchmark {
    static let runs = 3

    private static let challengeBytes = 32

    static func measure(
        _ statement: Statement,
        inputSize: InputSize?,
        onRun: @MainActor @Sendable (Int) -> Void
    ) async throws -> Measurement {
        switch (statement, inputSize) {
        case (.keccak, let size?):
            return try await measure(with: KeccakSession(messageBytes: size.bytes), onRun: onRun)
        case (.sha256, let size?):
            return try await measure(with: Sha256Session(messageBytes: size.bytes), onRun: onRun)
        case (.keccak, nil), (.sha256, nil):
            throw StatementUnavailable(reason: "Choose an input size for \(statement.rawValue).")
        case (.rsa, _):
            return try await measure(with: RsaSession(), onRun: onRun)
        case (.mlDsa, _):
            #if compiler(>=6.2)
            if #available(iOS 26.0, *) {
                return try await measure(with: MlDsaSession(), onRun: onRun)
            }
            #endif

            throw StatementUnavailable(reason: statement.unavailableReason ?? "ML-DSA-65 is unavailable")
        }
    }

    private static func measure<Session: StatementSession>(
        with session: Session,
        onRun: @MainActor @Sendable (Int) -> Void
    ) async throws -> Measurement {
        var samples: [Sample] = []

        for run in 1...runs {
            await onRun(run)

            let challenge = Data((0..<challengeBytes).map { _ in UInt8.random(in: .min ... .max) })
            let inputs = try session.inputs(for: challenge)

            let callStart = ProcessInfo.processInfo.systemUptime

            let proof = try await Session.Prover.prove(inputs: inputs, priority: .userInitiated)

            let wholeCall = ProcessInfo.processInfo.systemUptime - callStart

            let verify = try session.verify(proof, challenge: challenge)

            samples.append(
                Sample(
                    programId: proof.programId,
                    proofBytes: proof.proofBytes.count,
                    witness: proof.witnessTime,
                    prove: proof.proveTime,
                    wholeCall: wholeCall,
                    verify: verify
                )
            )
        }

        return Measurement(keyStorage: session.keyStorage, samples: samples)
    }
}