// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples.signatures

import android.content.Context
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import java.io.File

data class DeviceConditions(
    val model: String,
    val isEmulator: Boolean,
    val system: String,
    val powerSaveMode: Boolean,
    val power: String,
    val thermalStatus: String,
) {
    companion object {
        private val EMULATOR_HARDWARE = setOf("ranchu", "goldfish")

        fun current(context: Context): DeviceConditions {
            val battery = context.getSystemService(BatteryManager::class.java)
            val power = context.getSystemService(PowerManager::class.java)

            return DeviceConditions(
                model = "${Build.MANUFACTURER} ${Build.MODEL}, ${Build.SOC_MANUFACTURER} ${Build.SOC_MODEL}",
                isEmulator = Build.HARDWARE in EMULATOR_HARDWARE,
                system = "Android ${Build.VERSION.RELEASE}, API ${Build.VERSION.SDK_INT}, build ${Build.ID}",
                powerSaveMode = power.isPowerSaveMode,
                power = when (battery.isCharging) {
                    true -> "charging"
                    false -> "on battery"
                },
                thermalStatus = currentThermalStatus(context),
            )
        }

        fun currentThermalStatus(context: Context): String {
            val status = context.getSystemService(PowerManager::class.java).currentThermalStatus

            return when (status) {
                PowerManager.THERMAL_STATUS_NONE -> "none"
                PowerManager.THERMAL_STATUS_LIGHT -> "light"
                PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
                PowerManager.THERMAL_STATUS_SEVERE -> "severe"
                PowerManager.THERMAL_STATUS_CRITICAL -> "critical"
                PowerManager.THERMAL_STATUS_EMERGENCY -> "emergency"
                PowerManager.THERMAL_STATUS_SHUTDOWN -> "shutdown"
                else -> "unknown ($status)"
            }
        }
    }
}

data class Memory(val residentKib: Long, val peakResidentKib: Long) {
    companion object {
        private const val STATUS = "/proc/self/status"

        fun read(): Memory {
            val fields = File(STATUS).readLines().associate { line ->
                line.substringBefore(':') to line.substringAfter(':').trim()
            }

            return Memory(
                residentKib = kibibytes(fields, "VmRSS"),
                peakResidentKib = kibibytes(fields, "VmHWM"),
            )
        }

        private fun kibibytes(fields: Map<String, String>, name: String): Long =
            fields[name]?.removeSuffix("kB")?.trim()?.toLongOrNull()
                ?: throw IllegalStateException("$STATUS has no $name in kB")
    }
}