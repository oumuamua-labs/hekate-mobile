// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation
import Hekate

extension CancelToken: CancellableToken {}

enum RsaProver: HekateProver {
    static func makeCancelToken() -> CancelToken {
        CancelToken()
    }

    static func prove(inputs: RsaInputs, cancel: CancelToken) throws -> Proof {
        try proveRsa(inputs: inputs, cancel: cancel)
    }
}

enum MlDsaProver: HekateProver {
    static func makeCancelToken() -> CancelToken {
        CancelToken()
    }

    static func prove(inputs: MlDsaInputs, cancel: CancelToken) throws -> Proof {
        try proveMldsa(inputs: inputs, cancel: cancel)
    }
}

enum KeccakProver: HekateProver {
    static func makeCancelToken() -> CancelToken {
        CancelToken()
    }

    static func prove(inputs: Data, cancel: CancelToken) throws -> Proof {
        try proveKeccak(message: inputs, cancel: cancel)
    }
}

enum Sha256Prover: HekateProver {
    static func makeCancelToken() -> CancelToken {
        CancelToken()
    }

    static func prove(inputs: Data, cancel: CancelToken) throws -> Proof {
        try proveSha256(message: inputs, cancel: cancel)
    }
}