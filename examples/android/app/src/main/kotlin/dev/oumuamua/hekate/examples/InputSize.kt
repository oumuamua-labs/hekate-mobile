// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples

import java.math.BigDecimal
import java.math.MathContext

data class InputSize(val bytes: Int, val label: String) {
    companion object {
        private const val KIB = 1024
        private const val MIB = 1_048_576

        private val FULL_TRACE_LOG_ROWS = listOf<UByte>(15u, 20u)

        fun list(sizes: List<Int>, capacity: (UByte) -> ULong): List<InputSize> {
            val fixed = sizes.map { InputSize(it, describe(it)) }

            val fullTraces = FULL_TRACE_LOG_ROWS.map { logRows ->
                val bytes = capacity(logRows).toInt()

                InputSize(bytes, "2^$logRows rows, ${describe(bytes)}")
            }

            return fixed + fullTraces
        }

        private fun describe(bytes: Int): String = when {
            bytes < KIB -> "$bytes B"
            bytes < MIB -> "${significant(bytes, KIB, digits = 4)} KiB"
            else -> "${significant(bytes, MIB, digits = 3)} MiB"
        }

        private fun significant(bytes: Int, unit: Int, digits: Int): String =
            BigDecimal(bytes)
                .divide(BigDecimal(unit), MathContext(digits))
                .stripTrailingZeros()
                .toPlainString()
    }
}