// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import CryptoKit
import Foundation
import Hekate
import Security

enum Message {
    static func expand(_ seed: Data, to count: Int) -> Data {
        var message = Data(capacity: count + SHA256.byteCount)
        var counter: UInt64 = 0

        while message.count < count {
            var block = seed

            withUnsafeBytes(of: counter.littleEndian) { block.append(contentsOf: $0) }
            message.append(contentsOf: SHA256.hash(data: block))

            counter += 1
        }

        return Data(message.prefix(count))
    }
}

protocol StatementSession {
    associatedtype Prover: HekateProver where Prover.Output == Proof

    var keyStorage: String { get }

    func inputs(for challenge: Data) throws -> Prover.Inputs
    func verify(_ proof: Proof, challenge: Data) throws -> TimeInterval
}

struct KeccakSession: StatementSession {
    typealias Prover = KeccakProver

    let keyStorage = "none, the witness is a message"
    let messageBytes: Int

    func inputs(for challenge: Data) -> Data {
        Message.expand(challenge, to: messageBytes)
    }

    func verify(_ proof: Proof, challenge: Data) throws -> TimeInterval {
        try verifyKeccak(
            programId: proof.programId,
            proofBytes: proof.proofBytes,
            digest: keccak256(message: Message.expand(challenge, to: messageBytes)),
            messageBytes: UInt64(messageBytes)
        )
    }
}

struct Sha256Session: StatementSession {
    typealias Prover = Sha256Prover

    let keyStorage = "none, the witness is a message"
    let messageBytes: Int

    func inputs(for challenge: Data) -> Data {
        Message.expand(challenge, to: messageBytes)
    }

    func verify(_ proof: Proof, challenge: Data) throws -> TimeInterval {
        try verifySha256(
            programId: proof.programId,
            proofBytes: proof.proofBytes,
            digest: Data(SHA256.hash(data: Message.expand(challenge, to: messageBytes))),
            messageBytes: UInt64(messageBytes)
        )
    }
}

struct RsaSession: StatementSession {
    typealias Prover = RsaProver

    let keyStorage = "Keychain"

    private let key: SecKey
    private let modulus: Data

    init() throws {
        key = try RsaSigner.loadOrCreateKey()
        modulus = try RsaSigner.modulus(of: key)
    }

    func inputs(for challenge: Data) throws -> RsaInputs {
        RsaInputs(
            modulus: modulus,
            message: challenge,
            signature: try RsaSigner.sign(challenge, with: key)
        )
    }

    func verify(_ proof: Proof, challenge: Data) throws -> TimeInterval {
        try verifyRsa(programId: proof.programId, proofBytes: proof.proofBytes, modulus: modulus)
    }
}

#if compiler(>=6.2)
@available(iOS 26.0, macOS 26.0, *)
struct MlDsaSession: StatementSession {
    typealias Prover = MlDsaProver

    let keyStorage: String

    private let key: MlDsaKey
    private let publicKey: Data
    private let keyHash: Data

    init() throws {
        key = try MlDsaKey.loadOrCreate()
        publicKey = key.publicKey
        keyHash = mldsaKeyHash(publicKey: publicKey)
        keyStorage = key.isInSecureEnclave ? "Secure Enclave" : "Keychain, software key"
    }

    func inputs(for challenge: Data) throws -> MlDsaInputs {
        MlDsaInputs(publicKey: publicKey, message: challenge, signature: try key.sign(challenge))
    }

    func verify(_ proof: Proof, challenge: Data) throws -> TimeInterval {
        try verifyMldsa(
            programId: proof.programId,
            proofBytes: proof.proofBytes,
            message: challenge,
            keyHash: keyHash
        )
    }
}
#endif