// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation

struct Report: Sendable {
    struct Row: Identifiable, Sendable {
        let label: String
        let value: String

        var id: String { label }
    }

    let statement: Statement
    let before: DeviceConditions
    let thermalStateAfter: String
    let hekateVersion: String
    let prover: ProverBuild
    let measurement: Measurement
    let footprintBefore: Footprint
    let footprintAfter: Footprint

    var verdict: String {
        "\(measurement.samples.count) of \(Benchmark.runs) proofs verified"
    }

    var rows: [Row] {
        [
            Row(label: "Statement", value: statement.title),
            Row(label: "Witness", value: milliseconds(\.witness)),
            Row(label: "Prove", value: milliseconds(\.prove)),
            Row(label: "Whole call", value: milliseconds(\.wholeCall)),
            Row(label: "Verify", value: milliseconds(\.verify)),
            Row(label: "Proof", value: proofSize),
            Row(label: "Peak footprint", value: "\(mebibytes(footprintAfter.peakBytesSinceLaunch)) since launch"),
            Row(label: "Footprint before proving", value: mebibytes(footprintBefore.currentBytes)),
            Row(label: "Key", value: measurement.keyStorage),
            Row(label: "Device", value: before.isSimulator ? "\(before.model), simulator on a Mac" : before.model),
            Row(label: "System", value: before.system),
            Row(label: "Hekate", value: hekateVersion),
            Row(label: "Prover", value: "\(prover.version), \(prover.mode)"),
            Row(label: "Thermal state", value: "\(before.thermalState) before, \(thermalStateAfter) after"),
            Row(label: "Low Power Mode", value: before.lowPowerMode ? "on" : "off"),
            Row(label: "Power", value: before.power),
            Row(label: "Program", value: measurement.samples.first?.programId ?? "none"),
        ]
    }

    var text: String {
        ([verdict] + rows.map { "\($0.label): \($0.value)" }).joined(separator: "\n")
    }

    private var proofSize: String {
        let sizes = measurement.samples.map(\.proofBytes)

        guard let smallest = sizes.min(), let largest = sizes.max() else {
            return "none"
        }

        return String(format: "%.0f to %.0f KiB", Double(smallest) / 1024, Double(largest) / 1024)
    }

    private func milliseconds(_ time: KeyPath<Sample, TimeInterval>) -> String {
        let times = measurement.samples.map { $0[keyPath: time] * 1000 }

        guard let best = times.min(), let worst = times.max() else {
            return "none"
        }

        return String(format: "%.1f ms best of %d, %.1f ms worst", best, times.count, worst)
    }

    private func mebibytes<Bytes: BinaryInteger>(_ bytes: Bytes) -> String {
        String(format: "%.0f MiB", Double(bytes) / 1_048_576)
    }
}