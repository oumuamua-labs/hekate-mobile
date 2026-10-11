// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation
import Security

enum RsaSignerError: LocalizedError {
    case security(String)
    case publicKeyEncoding(String)
    case unsupportedExponent

    var errorDescription: String? {
        switch self {
        case .security(let message):
            return "Security framework: \(message)"
        case .publicKeyEncoding(let message):
            return "public key encoding: \(message)"
        case .unsupportedExponent:
            return "the public exponent is not 65537, the only one the statement proves"
        }
    }
}

enum RsaSigner {
    static let modulusBytes = 256

    private static let keyTag = Data("dev.oumuamua.hekate.examples.signatures.rsa-2048".utf8)
    private static let publicExponent: [UInt8] = [0x01, 0x00, 0x01]

    static func loadOrCreateKey() throws -> SecKey {
        let query: [CFString: Any] = [
            kSecClass: kSecClassKey,
            kSecAttrApplicationTag: keyTag,
            kSecAttrKeyType: kSecAttrKeyTypeRSA,
            kSecAttrKeyClass: kSecAttrKeyClassPrivate,
            kSecReturnRef: true,
        ]

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)

        switch status {
        case errSecSuccess:
            guard let item, CFGetTypeID(item) == SecKeyGetTypeID() else {
                throw KeychainError.unexpectedItem
            }

            return item as! SecKey
        case errSecItemNotFound:
            return try createKey()
        default:
            throw KeychainError.status(status)
        }
    }

    static func sign(_ message: Data, with key: SecKey) throws -> Data {
        var error: Unmanaged<CFError>?

        guard let signature = SecKeyCreateSignature(
            key,
            .rsaSignatureMessagePKCS1v15SHA256,
            message as CFData,
            &error
        ) else {
            throw securityError(error, during: "SecKeyCreateSignature")
        }

        return signature as Data
    }

    static func modulus(of key: SecKey) throws -> Data {
        guard let publicKey = SecKeyCopyPublicKey(key) else {
            throw RsaSignerError.security("SecKeyCopyPublicKey returned no key")
        }

        var error: Unmanaged<CFError>?

        guard let encoded = SecKeyCopyExternalRepresentation(publicKey, &error) else {
            throw securityError(error, during: "SecKeyCopyExternalRepresentation")
        }

        return try modulus(fromPkcs1PublicKey: encoded as Data)
    }

    static func modulus(fromPkcs1PublicKey encoded: Data) throws -> Data {
        var outer = DerReader(encoded[...])
        var sequence = DerReader(try outer.read(.sequence))

        let modulus = try sequence.read(.integer)
        let exponent = try sequence.read(.integer)

        guard outer.isEmpty, sequence.isEmpty else {
            throw RsaSignerError.publicKeyEncoding("trailing bytes after RSAPublicKey")
        }

        guard exponent.elementsEqual(publicExponent) else {
            throw RsaSignerError.unsupportedExponent
        }

        let magnitude = modulus.first == 0x00 ? modulus.dropFirst() : modulus

        guard magnitude.count == modulusBytes, let top = magnitude.first, top & 0x80 != 0 else {
            throw RsaSignerError.publicKeyEncoding("the modulus is not 2048 bits")
        }

        return Data(magnitude)
    }

    private static func createKey() throws -> SecKey {
        let privateKeyAttributes: [CFString: Any] = [
            kSecAttrIsPermanent: true,
            kSecAttrApplicationTag: keyTag,
            kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        ]

        let attributes: [CFString: Any] = [
            kSecAttrKeyType: kSecAttrKeyTypeRSA,
            kSecAttrKeySizeInBits: modulusBytes * 8,
            kSecPrivateKeyAttrs: privateKeyAttributes,
        ]

        var error: Unmanaged<CFError>?

        guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else {
            throw securityError(error, during: "SecKeyCreateRandomKey")
        }

        return key
    }

    private static func securityError(_ error: Unmanaged<CFError>?, during call: String) -> RsaSignerError {
        guard let error = error?.takeRetainedValue() else {
            return .security("\(call) failed without an error")
        }

        return .security("\(call): \(CFErrorCopyDescription(error) as String)")
    }
}

private enum DerTag: UInt8 {
    case integer = 0x02
    case sequence = 0x30
}

private struct DerReader {
    private var rest: Data

    init(_ bytes: Data) {
        rest = bytes
    }

    var isEmpty: Bool { rest.isEmpty }

    mutating func read(_ expected: DerTag) throws -> Data {
        guard let tag = rest.popFirst(), tag == expected.rawValue else {
            throw RsaSignerError.publicKeyEncoding("expected DER \(expected)")
        }

        let length = try readLength()

        guard length <= rest.count else {
            throw RsaSignerError.publicKeyEncoding("DER length \(length) runs past the end")
        }

        let content = rest.prefix(length)
        rest = rest.dropFirst(length)

        return content
    }

    private mutating func readLength() throws -> Int {
        guard let first = rest.popFirst() else {
            throw RsaSignerError.publicKeyEncoding("missing DER length")
        }

        if first < 0x80 {
            return Int(first)
        }

        let octets = Int(first & 0x7f)

        guard (1...2).contains(octets), rest.count >= octets else {
            throw RsaSignerError.publicKeyEncoding("unsupported DER length form")
        }

        let length = rest.prefix(octets).reduce(0) { $0 << 8 | Int($1) }
        rest = rest.dropFirst(octets)

        return length
    }
}