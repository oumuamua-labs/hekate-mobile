// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples.signatures

import dev.oumuamua.hekate.CancelToken
import dev.oumuamua.hekate.HekateProver
import uniffi.signature_prover.MlDsaInputs
import uniffi.signature_prover.Proof
import uniffi.signature_prover.RsaInputs
import uniffi.signature_prover.proveMldsa
import uniffi.signature_prover.proveRsa

class ProverToken : CancelToken {
    val generated = uniffi.signature_prover.CancelToken()

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