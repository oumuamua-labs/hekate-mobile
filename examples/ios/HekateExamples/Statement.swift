// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation

struct StatementUnavailable: LocalizedError {
    let reason: String

    var errorDescription: String? { reason }
}

enum Statement: String, CaseIterable, Identifiable, Sendable {
    case rsa = "RSA-2048"
    case mlDsa = "ML-DSA-65"
    case keccak = "Keccak-256"
    case sha256 = "SHA-256"

    var id: Self { self }

    var title: String {
        switch self {
        case .rsa: return "RSA-2048 PKCS#1 v1.5, SHA-256"
        case .mlDsa: return "ML-DSA-65, FIPS 204"
        case .keccak: return "Keccak-256 preimage"
        case .sha256: return "SHA-256 preimage"
        }
    }

    var inputSizes: [InputSize] {
        switch self {
        case .rsa, .mlDsa:
            return []
        case .keccak:
            return InputSize.list(
                bytes: [128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 262_144],
                capacity: { keccakCapacity(logRows: $0) }
            )
        case .sha256:
            return InputSize.list(
                bytes: [128, 256, 512, 1024, 2048],
                capacity: { sha256Capacity(logRows: $0) }
            )
        }
    }

    var claim: String {
        switch self {
        case .rsa:
            return "A PKCS#1 v1.5 signature with SHA-256 over some message "
                + "verifies under the RSA-2048 modulus N."
        case .mlDsa:
            return "An ML-DSA-65 signature over the message verifies under a public key "
                + "whose hash is tr."
        case .keccak:
            return "A message of the given length hashes to the digest under Keccak-256."
        case .sha256:
            return "A message of the given length hashes to the digest under SHA-256."
        }
    }

    var publicValues: String {
        switch self {
        case .rsa: return "the modulus N"
        case .mlDsa: return "the message and the key hash tr"
        case .keccak, .sha256: return "the digest and the message length"
        }
    }

    var hiddenValues: String {
        switch self {
        case .rsa: return "the message and the signature"
        case .mlDsa: return "the public key and the signature"
        case .keccak, .sha256: return "the message"
        }
    }

    var unavailableReason: String? {
        switch self {
        case .rsa, .keccak, .sha256:
            return nil
        case .mlDsa:
            #if compiler(>=6.2)
            if #available(iOS 26.0, *) {
                return nil
            }

            return "ML-DSA-65 signing needs iOS 26."
            #else
            return "ML-DSA-65 signing needs the iOS 26 SDK. Build the app with Xcode 26."
            #endif
        }
    }
}