// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

package dev.oumuamua.hekate

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.cancellation.CancellationException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class TestToken : CancelToken {
    private val flag = AtomicBoolean(false)

    val requested: Boolean get() = flag.get()

    override fun request() {
        flag.set(true)
    }
}

class Boom : RuntimeException("boom")

class HekateProverTest {
    @Test
    fun async_prove_returns_value_from_sync_impl() = runBlocking {
        val prover = object : HekateProver<Int, Int, TestToken> {
            override fun makeCancelToken() = TestToken()
            override fun prove(inputs: Int, cancel: TestToken): Int = inputs * 2
        }

        assertEquals(42, prover.prove(21))
    }

    @Test
    fun sync_prove_error_surfaces_on_async_path() = runBlocking {
        val prover = object : HekateProver<Unit, Unit, TestToken> {
            override fun makeCancelToken() = TestToken()
            override fun prove(inputs: Unit, cancel: TestToken): Unit = throw Boom()
        }

        assertFailsWith<Boom> { prover.prove(Unit) }
    }

    @Test
    fun cancel_propagates_to_token_request() = runBlocking {
        val token = TestToken()
        val started = CompletableDeferred<Unit>()

        val prover = object : HekateProver<Unit, Unit, TestToken> {
            override fun makeCancelToken() = token
            override fun prove(inputs: Unit, cancel: TestToken) {
                started.complete(Unit)
                while (!cancel.requested) {
                    Thread.sleep(5)
                }
                
                throw CancellationException("cancelled by token")
            }
        }

        val job = launch(Dispatchers.Default) {
            prover.prove(Unit)
        }

        started.await()
        job.cancel()
        withTimeout(2_000) { job.join() }

        assertTrue(token.requested, "Job.cancel() must trigger token.request()")
    }
}