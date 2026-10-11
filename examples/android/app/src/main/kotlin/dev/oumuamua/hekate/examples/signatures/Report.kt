// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples.signatures

import java.time.Duration
import java.util.Locale
import uniffi.signature_prover.ProverBuild

data class Report(
    val statement: Statement,
    val before: DeviceConditions,
    val thermalStatusAfter: String,
    val hekateVersion: String,
    val prover: ProverBuild,
    val measurement: Measurement,
    val memoryBefore: Memory,
    val memoryAfter: Memory,
) {
    val verdict: String
        get() = "${measurement.samples.size} of ${Benchmark.RUNS} proofs verified"

    val text: String
        get() = rows().joinToString("\n") { (label, value) -> "$label: $value" }

    private fun rows(): List<Pair<String, String>> = listOf(
        "Statement" to statement.title,
        "Witness" to milliseconds(Sample::witness),
        "Prove" to milliseconds(Sample::prove),
        "Whole call" to milliseconds(Sample::wholeCall),
        "Verify" to milliseconds(Sample::verify),
        "Proof" to proofSize(),
        "Peak resident memory" to "${mebibytes(memoryAfter.peakResidentKib)} since launch",
        "Resident memory before proving" to mebibytes(memoryBefore.residentKib),
        "Key" to measurement.keyStorage,
        "Device" to when (before.isEmulator) {
            true -> "${before.model}, emulator on a host computer"
            false -> before.model
        },
        "System" to before.system,
        "Hekate" to hekateVersion,
        "Prover" to "${prover.version}, ${prover.mode}",
        "Thermal status" to "${before.thermalStatus} before, $thermalStatusAfter after",
        "Battery Saver" to when (before.powerSaveMode) {
            true -> "on"
            false -> "off"
        },
        "Power" to before.power,
        "Program" to (measurement.samples.firstOrNull()?.programId ?: "none"),
    )

    private fun milliseconds(time: (Sample) -> Duration): String {
        val times = measurement.samples.map { time(it).toNanos() / 1e6 }

        val best = times.minOrNull() ?: return "none"
        val worst = times.maxOrNull() ?: return "none"

        return String.format(Locale.ROOT, "%.1f ms best of %d, %.1f ms worst", best, times.size, worst)
    }

    private fun proofSize(): String {
        val sizes = measurement.samples.map { it.proofBytes / 1024.0 }

        val smallest = sizes.minOrNull() ?: return "none"
        val largest = sizes.maxOrNull() ?: return "none"

        return String.format(Locale.ROOT, "%.0f to %.0f KiB", smallest, largest)
    }

    private fun mebibytes(kibibytes: Long): String =
        String.format(Locale.ROOT, "%.0f MiB", kibibytes / 1024.0)
}