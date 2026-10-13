// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples

import dev.oumuamua.hekate.prove
import java.security.SecureRandom
import java.time.Duration
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

data class Sample(
    val programId: String,
    val proofBytes: Int,
    val witness: Duration,
    val prove: Duration,
    val wholeCall: Duration,
    val verify: Duration,
)

data class Measurement(val keyStorage: String, val samples: List<Sample>)

object Benchmark {
    const val RUNS = 3

    private const val CHALLENGE_BYTES = 32

    suspend fun measure(
        statement: Statement,
        inputSize: InputSize?,
        onRun: (Int) -> Unit,
    ): Measurement =
        withContext(Dispatchers.Default) {
            when (statement) {
                Statement.RSA -> measure(RsaSession(), onRun)
                Statement.ML_DSA -> measure(MlDsaSession(), onRun)
                Statement.KECCAK -> measure(KeccakSession(messageBytes(statement, inputSize)), onRun)
                Statement.SHA256 -> measure(Sha256Session(messageBytes(statement, inputSize)), onRun)
            }
        }

    private fun messageBytes(statement: Statement, inputSize: InputSize?): Int =
        requireNotNull(inputSize) { "Choose an input size for ${statement.title}." }.bytes

    private suspend fun <Inputs> measure(
        session: StatementSession<Inputs>,
        onRun: (Int) -> Unit,
    ): Measurement {
        val random = SecureRandom()

        val samples = (1..RUNS).map { run ->
            withContext(Dispatchers.Main) { onRun(run) }

            val challenge = ByteArray(CHALLENGE_BYTES).also(random::nextBytes)
            val inputs = session.inputs(challenge)

            val callStart = System.nanoTime()

            val proof = session.prover.prove(inputs)

            val wholeCall = Duration.ofNanos(System.nanoTime() - callStart)

            val verify = session.verify(proof, challenge)

            Sample(
                programId = proof.programId,
                proofBytes = proof.proofBytes.size,
                witness = proof.witnessTime,
                prove = proof.proveTime,
                wholeCall = wholeCall,
                verify = verify,
            )
        }

        return Measurement(session.keyStorage, samples)
    }
}