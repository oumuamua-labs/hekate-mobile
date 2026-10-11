// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples.signatures

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
    );

    fun unavailableReason(): String? = when (this) {
        RSA -> null
        ML_DSA -> when (MlDsaSession.isSupported()) {
            true -> null
            false -> "The platform library of this Android has no ML-DSA-65."
        }
    }
}