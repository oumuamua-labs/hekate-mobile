// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation
import Hekate
import Security

protocol SignatureSession {
    associatedtype Prover: HekateProver where Prover.Output == Proof

    var keyStorage: String { get }

    func sign(_ challenge: Data) throws -> Prover.Inputs
    func verify(_ proof: Proof, challenge: Data) throws -> TimeInterval
}

struct RsaSession: SignatureSession {
    typealias Prover = RsaProver

    let keyStorage = "Keychain"

    private let key: SecKey
    private let modulus: Data

    init() throws {
        key = try RsaSigner.loadOrCreateKey()
        modulus = try RsaSigner.modulus(of: key)
    }

    func sign(_ challenge: Data) throws -> RsaInputs {
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
struct MlDsaSession: SignatureSession {
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

    func sign(_ challenge: Data) throws -> MlDsaInputs {
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