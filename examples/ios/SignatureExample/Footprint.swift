// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Darwin
import Foundation

struct FootprintError: LocalizedError {
    let status: kern_return_t

    var errorDescription: String? {
        "task_info(TASK_VM_INFO) failed with kern_return_t \(status)"
    }
}

struct Footprint: Sendable {
    let currentBytes: UInt64
    let peakBytesSinceLaunch: Int64

    static func read() throws -> Footprint {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(
            MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size
        )

        let status = withUnsafeMutablePointer(to: &info) { pointer in
            pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { words in
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), words, &count)
            }
        }

        guard status == KERN_SUCCESS else {
            throw FootprintError(status: status)
        }

        return Footprint(
            currentBytes: info.phys_footprint,
            peakBytesSinceLaunch: info.ledger_phys_footprint_peak
        )
    }
}