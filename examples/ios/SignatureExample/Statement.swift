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

    var id: Self { self }

    var title: String {
        switch self {
        case .rsa: return "RSA-2048 PKCS#1 v1.5, SHA-256"
        case .mlDsa: return "ML-DSA-65, FIPS 204"
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
        }
    }

    var publicValues: String {
        switch self {
        case .rsa: return "the modulus N"
        case .mlDsa: return "the message and the key hash tr"
        }
    }

    var hiddenValues: String {
        switch self {
        case .rsa: return "the message and the signature"
        case .mlDsa: return "the public key and the signature"
        }
    }

    var unavailableReason: String? {
        switch self {
        case .rsa:
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