// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

#if compiler(>=6.2)
import CryptoKit
import Foundation

@available(iOS 26.0, macOS 26.0, *)
enum MlDsaKey {
    case secureEnclave(SecureEnclave.MLDSA65.PrivateKey)
    case software(MLDSA65.PrivateKey)

    private static let secureEnclaveAccount = "ml-dsa-65.secure-enclave"
    private static let softwareAccount = "ml-dsa-65.seed"

    var isInSecureEnclave: Bool {
        switch self {
        case .secureEnclave: return true
        case .software: return false
        }
    }

    var publicKey: Data {
        switch self {
        case .secureEnclave(let key): return key.publicKey.rawRepresentation
        case .software(let key): return key.publicKey.rawRepresentation
        }
    }

    static func loadOrCreate() throws -> MlDsaKey {
        if let key = try load() {
            return key
        }

        let key = try create()

        switch key {
        case .secureEnclave(let enclaveKey):
            try KeychainBlob.store(enclaveKey.dataRepresentation, account: secureEnclaveAccount)
        case .software(let softwareKey):
            try KeychainBlob.store(softwareKey.seedRepresentation, account: softwareAccount)
        }

        return key
    }

    static func create() throws -> MlDsaKey {
        guard SecureEnclave.isAvailable else {
            return .software(try MLDSA65.PrivateKey())
        }

        return .secureEnclave(try SecureEnclave.MLDSA65.PrivateKey())
    }

    func sign(_ message: Data) throws -> Data {
        switch self {
        case .secureEnclave(let key): return try key.signature(for: message)
        case .software(let key): return try key.signature(for: message)
        }
    }

    private static func load() throws -> MlDsaKey? {
        guard SecureEnclave.isAvailable else {
            return try KeychainBlob.load(account: softwareAccount).map { seed in
                .software(try MLDSA65.PrivateKey(seedRepresentation: seed, publicKey: nil))
            }
        }

        return try KeychainBlob.load(account: secureEnclaveAccount).map { blob in
            .secureEnclave(try SecureEnclave.MLDSA65.PrivateKey(dataRepresentation: blob))
        }
    }
}
#endif