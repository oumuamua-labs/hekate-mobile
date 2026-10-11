// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples.signatures

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.util.Log
import dev.oumuamua.hekate.HekateProver
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.NoSuchAlgorithmException
import java.security.PrivateKey
import java.security.ProviderException
import java.security.PublicKey
import java.security.Security
import java.security.Signature
import java.security.interfaces.RSAPublicKey
import java.security.spec.RSAKeyGenParameterSpec
import java.time.Duration
import uniffi.signature_prover.MlDsaInputs
import uniffi.signature_prover.Proof
import uniffi.signature_prover.RsaInputs
import uniffi.signature_prover.mldsaKeyHash
import uniffi.signature_prover.verifyMldsa
import uniffi.signature_prover.verifyRsa

private const val KEYSTORE = "AndroidKeyStore"
private const val LOG_TAG = "HekateSignatures"

class SignerException(message: String) : Exception(message)

interface SignatureSession<Inputs> {
    val prover: HekateProver<Inputs, Proof, ProverToken>
    val keyStorage: String

    fun sign(challenge: ByteArray): Inputs

    fun verify(proof: Proof, challenge: ByteArray): Duration
}

class RsaSession : SignatureSession<RsaInputs> {
    override val prover = RsaProver
    override val keyStorage: String

    private val key: PrivateKey
    private val modulus: ByteArray

    init {
        val pair = loadKeyPair(ALIAS) ?: generate()
        val publicKey = pair.public as? RSAPublicKey
            ?: throw SignerException("the Keystore returned a public key that is not RSA")

        if (publicKey.publicExponent != RSAKeyGenParameterSpec.F4) {
            throw SignerException("the public exponent is not 65537, the only one the statement proves")
        }

        key = pair.private
        modulus = unsignedBytes(publicKey)
        keyStorage = "Keystore, ${securityLevel(key)}"
    }

    override fun sign(challenge: ByteArray): RsaInputs {
        val signature = Signature.getInstance("SHA256withRSA").run {
            initSign(key)
            update(challenge)
            sign()
        }

        return RsaInputs(modulus = modulus, message = challenge, signature = signature)
    }

    override fun verify(proof: Proof, challenge: ByteArray): Duration =
        verifyRsa(proof.programId, proof.proofBytes, modulus)

    private fun generate(): KeyPair {
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_SIGN)
            .setAlgorithmParameterSpec(RSAKeyGenParameterSpec(MODULUS_BITS, RSAKeyGenParameterSpec.F4))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setSignaturePaddings(KeyProperties.SIGNATURE_PADDING_RSA_PKCS1)
            .build()

        return KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, KEYSTORE).run {
            initialize(spec)
            generateKeyPair()
        }
    }

    private fun unsignedBytes(publicKey: RSAPublicKey): ByteArray {
        if (publicKey.modulus.bitLength() != MODULUS_BITS) {
            throw SignerException("the modulus is ${publicKey.modulus.bitLength()} bits, not $MODULUS_BITS")
        }

        return publicKey.modulus.toByteArray().dropWhile { it == 0.toByte() }.toByteArray()
    }

    private companion object {
        const val ALIAS = "hekate-example-rsa-2048"
        const val MODULUS_BITS = 2048
    }
}

class MlDsaSession : SignatureSession<MlDsaInputs> {
    override val prover = MlDsaProver
    override val keyStorage: String

    private val key: PrivateKey
    private val publicKey: ByteArray
    private val keyHash: ByteArray

    init {
        val keystorePair = loadKeyPair(ALIAS) ?: generateInKeystore()

        val pair = keystorePair ?: KeyPairGenerator.getInstance(ALGORITHM, PLATFORM_PROVIDER).generateKeyPair()

        key = pair.private
        publicKey = rawPublicKey(pair.public)
        keyHash = mldsaKeyHash(publicKey)

        keyStorage = when (keystorePair) {
            null -> "platform software key in memory, the Keystore made no ML-DSA-65 key"
            else -> "Keystore, ${securityLevel(key)}"
        }
    }

    override fun sign(challenge: ByteArray): MlDsaInputs {
        val signature = Signature.getInstance(ALGORITHM).run {
            initSign(key)
            update(challenge)
            sign()
        }

        return MlDsaInputs(publicKey = publicKey, message = challenge, signature = signature)
    }

    override fun verify(proof: Proof, challenge: ByteArray): Duration =
        verifyMldsa(proof.programId, proof.proofBytes, challenge, keyHash)

    private fun generateInKeystore(): KeyPair? {
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_SIGN).build()

        return try {
            KeyPairGenerator.getInstance(ALGORITHM, KEYSTORE).run {
                initialize(spec)
                generateKeyPair()
            }
        } catch (e: NoSuchAlgorithmException) {
            Log.w(LOG_TAG, "the Keystore has no ML-DSA-65 key generator", e)

            null
        } catch (e: ProviderException) {
            Log.w(LOG_TAG, "the Keystore made no ML-DSA-65 key", e)

            null
        }
    }

    private fun rawPublicKey(publicKey: PublicKey): ByteArray {
        val encoded = publicKey.encoded
            ?: throw SignerException("the ML-DSA-65 public key has no encoding")

        val prefix = encoded.take(SPKI_PREFIX.size)

        if (encoded.size != SPKI_PREFIX.size + PUBLIC_KEY_BYTES || prefix != SPKI_PREFIX.toList()) {
            throw SignerException("the public key is not an ML-DSA-65 SubjectPublicKeyInfo")
        }

        return encoded.copyOfRange(SPKI_PREFIX.size, encoded.size)
    }

    companion object {
        private const val ALIAS = "hekate-example-ml-dsa-65"
        private const val ALGORITHM = "ML-DSA-65"
        private const val PLATFORM_PROVIDER = "AndroidOpenSSL"
        private const val PUBLIC_KEY_BYTES = 1952

        private val SPKI_PREFIX = intArrayOf(
            0x30, 0x82, 0x07, 0xb2, 0x30, 0x0b, 0x06, 0x09, 0x60, 0x86, 0x48,
            0x01, 0x65, 0x03, 0x04, 0x03, 0x12, 0x03, 0x82, 0x07, 0xa1, 0x00,
        ).map(Int::toByte).toByteArray()

        fun isSupported(): Boolean =
            Security.getProvider(PLATFORM_PROVIDER)?.getService("Signature", ALGORITHM) != null
    }
}

private fun loadKeyPair(alias: String): KeyPair? {
    val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    val key = store.getKey(alias, null) as? PrivateKey ?: return null
    val certificate = store.getCertificate(alias)
        ?: throw SignerException("the Keystore has the key $alias without its certificate")

    return KeyPair(certificate.publicKey, key)
}

private fun securityLevel(key: PrivateKey): String {
    val info = KeyFactory.getInstance(key.algorithm, KEYSTORE).getKeySpec(key, KeyInfo::class.java)

    return when (info.securityLevel) {
        KeyProperties.SECURITY_LEVEL_STRONGBOX -> "StrongBox"
        KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT -> "trusted execution environment"
        KeyProperties.SECURITY_LEVEL_SOFTWARE -> "software"
        else -> "security level ${info.securityLevel}"
    }
}