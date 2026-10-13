// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples

import dev.oumuamua.hekate.CancelToken
import dev.oumuamua.hekate.HekateProver
import uniffi.example_prover.MlDsaInputs
import uniffi.example_prover.Proof
import uniffi.example_prover.RsaInputs
import uniffi.example_prover.proveKeccak
import uniffi.example_prover.proveMldsa
import uniffi.example_prover.proveRsa
import uniffi.example_prover.proveSha256

class ProverToken : CancelToken {
    val generated = uniffi.example_prover.CancelToken()

    override fun request() {
        generated.request()
    }
}

object RsaProver : HekateProver<RsaInputs, Proof, ProverToken> {
    override fun makeCancelToken() = ProverToken()

    override fun prove(inputs: RsaInputs, cancel: ProverToken): Proof =
        proveRsa(inputs, cancel.generated)
}

object MlDsaProver : HekateProver<MlDsaInputs, Proof, ProverToken> {
    override fun makeCancelToken() = ProverToken()

    override fun prove(inputs: MlDsaInputs, cancel: ProverToken): Proof =
        proveMldsa(inputs, cancel.generated)
}

object KeccakProver : HekateProver<ByteArray, Proof, ProverToken> {
    override fun makeCancelToken() = ProverToken()

    override fun prove(inputs: ByteArray, cancel: ProverToken): Proof =
        proveKeccak(inputs, cancel.generated)
}

object Sha256Prover : HekateProver<ByteArray, Proof, ProverToken> {
    override fun makeCancelToken() = ProverToken()

    override fun prove(inputs: ByteArray, cancel: ProverToken): Proof =
        proveSha256(inputs, cancel.generated)
}