// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation
import Security

enum KeychainError: LocalizedError {
    case status(OSStatus)
    case unexpectedItem

    var errorDescription: String? {
        switch self {
        case .status(let status):
            let text = SecCopyErrorMessageString(status, nil) as String? ?? "no description"
            return "keychain error \(status): \(text)"
        case .unexpectedItem:
            return "the keychain returned an item of another type"
        }
    }
}

enum KeychainBlob {
    private static let service = "dev.oumuamua.hekate.examples.signatures"

    static func load(account: String) throws -> Data? {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecReturnData: true,
        ]

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)

        switch status {
        case errSecSuccess:
            guard let data = item as? Data else {
                throw KeychainError.unexpectedItem
            }

            return data
        case errSecItemNotFound:
            return nil
        default:
            throw KeychainError.status(status)
        }
    }

    static func store(_ data: Data, account: String) throws {
        let attributes: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecValueData: data,
        ]

        let status = SecItemAdd(attributes as CFDictionary, nil)

        guard status == errSecSuccess else {
            throw KeychainError.status(status)
        }
    }
}