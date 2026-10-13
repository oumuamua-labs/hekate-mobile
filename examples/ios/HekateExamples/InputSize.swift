// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation

struct InputSize: Hashable, Identifiable, Sendable {
    let bytes: Int
    let label: String

    var id: Int { bytes }

    private static let fullTraceLogRows: [UInt8] = [15, 20]

    static func list(bytes sizes: [Int], capacity: (UInt8) -> UInt64) -> [InputSize] {
        let fixed = sizes.map { InputSize(bytes: $0, label: describe($0)) }

        let fullTraces = fullTraceLogRows.map { logRows in
            let bytes = Int(capacity(logRows))

            return InputSize(bytes: bytes, label: "2^\(logRows) rows, \(describe(bytes))")
        }

        return fixed + fullTraces
    }

    private static func describe(_ bytes: Int) -> String {
        switch bytes {
        case ..<1024:
            return "\(bytes) B"
        case ..<1_048_576:
            return String(format: "%.4g KiB", Double(bytes) / 1024)
        default:
            return String(format: "%.3g MiB", Double(bytes) / 1_048_576)
        }
    }
}