// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext

public interface HekateProver<Inputs, Output, Token : CancelToken> {
    public fun prove(inputs: Inputs, cancel: Token): Output
    public fun makeCancelToken(): Token
}

/** `dispatcher` defaults to `Default` (CPU pool, sized to cores).
 *  Pass `IO` or a dedicated pool for concurrent / background proving,
 *  `Default` will starve when multiple proofs run at once.
 *  `Job.cancel()` -> `token.request()` fires on cancel-requested,
 *  not on terminal state, Rust polls the flag and unblocks prove(). */
public suspend fun <Inputs, Output, Token : CancelToken> HekateProver<Inputs, Output, Token>.prove(
    inputs: Inputs,
    dispatcher: CoroutineDispatcher = Dispatchers.Default,
): Output = withContext(dispatcher) {
    val token = makeCancelToken()
    
    suspendCancellableCoroutine { cont ->
        cont.invokeOnCancellation { token.request() }
        cont.resumeWith(runCatching { prove(inputs, token) })
    }
}