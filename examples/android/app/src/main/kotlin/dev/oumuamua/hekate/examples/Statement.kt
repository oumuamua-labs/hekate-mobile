// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples

import uniffi.example_prover.keccakCapacity
import uniffi.example_prover.sha256Capacity

enum class Statement(
    val title: String,
    val claim: String,
    val publicValues: String,
    val hiddenValues: String,
) {
    RSA(
        title = "RSA-2048 PKCS#1 v1.5, SHA-256",
        claim = "A PKCS#1 v1.5 signature with SHA-256 over some message " +
            "verifies under the RSA-2048 modulus N.",
        publicValues = "the modulus N",
        hiddenValues = "the message and the signature",
    ),
    ML_DSA(
        title = "ML-DSA-65, FIPS 204",
        claim = "An ML-DSA-65 signature over the message verifies under a public key " +
            "whose hash is tr.",
        publicValues = "the message and the key hash tr",
        hiddenValues = "the public key and the signature",
    ),
    KECCAK(
        title = "Keccak-256 preimage",
        claim = "A message of the given length hashes to the digest under Keccak-256.",
        publicValues = "the digest and the message length",
        hiddenValues = "the message",
    ),
    SHA256(
        title = "SHA-256 preimage",
        claim = "A message of the given length hashes to the digest under SHA-256.",
        publicValues = "the digest and the message length",
        hiddenValues = "the message",
    );

    fun inputSizes(): List<InputSize> = when (this) {
        RSA, ML_DSA -> emptyList()
        KECCAK -> InputSize.list(
            listOf(128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 262_144),
            ::keccakCapacity,
        )
        SHA256 -> InputSize.list(listOf(128, 256, 512, 1024, 2048), ::sha256Capacity)
    }

    fun unavailableReason(): String? = when (this) {
        RSA, KECCAK, SHA256 -> null
        ML_DSA -> when (MlDsaSession.isSupported()) {
            true -> null
            false -> "The platform library of this Android has no ML-DSA-65."
        }
    }
}