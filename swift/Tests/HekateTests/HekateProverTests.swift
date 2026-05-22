// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

import XCTest
@testable import Hekate

final class HekateProverTests: XCTestCase {
    func testAsyncProveReturnsValueFromSyncImpl() async throws {
        struct Inputs: Sendable { let n: Int }
        struct Output: Sendable, Equatable { let doubled: Int }

        enum Mock: HekateProver {
            static func makeCancelToken() -> MockToken { MockToken() }

            static func prove(inputs: Inputs, cancel: MockToken) throws -> Output {
                Output(doubled: inputs.n * 2)
            }
        }

        let out = try await Mock.prove(inputs: Inputs(n: 21))
        XCTAssertEqual(out, Output(doubled: 42))
    }

    func testSyncProveErrorSurfacesOnAsyncPath() async {
        struct Inputs: Sendable {}
        struct Output: Sendable {}
        struct Boom: Error, Equatable {}

        enum Mock: HekateProver {
            static func makeCancelToken() -> MockToken { MockToken() }

            static func prove(inputs: Inputs, cancel: MockToken) throws -> Output {
                throw Boom()
            }
        }

        do {
            _ = try await Mock.prove(inputs: Inputs())
            XCTFail("expected throw")
        } catch let e as Boom {
            XCTAssertEqual(e, Boom())
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testTaskCancelTriggersTokenRequest() async {
        struct Inputs: Sendable {}
        struct Output: Sendable {}

        let started = AsyncEvent()
        let token = MockToken()

        enum Mock: HekateProver {
            nonisolated(unsafe) static var sharedToken: MockToken!
            nonisolated(unsafe) static var sharedStarted: AsyncEvent!

            static func makeCancelToken() -> MockToken { sharedToken }
            
            static func prove(inputs: Inputs, cancel: MockToken) throws -> Output {
                sharedStarted.signal()
                while !cancel.requested {
                    Thread.sleep(forTimeInterval: 0.01)
                }
                
                throw CancellationError()
            }
        }

        Mock.sharedToken = token
        Mock.sharedStarted = started

        let task = Task {
            try await Mock.prove(inputs: Inputs())
        }

        await started.wait()
        task.cancel()

        do {
            _ = try await task.value
            XCTFail("expected throw")
        } catch is CancellationError {
            XCTAssertTrue(token.requested, "cancel() must have triggered token.request()")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }
}

final class MockToken: CancellableToken, @unchecked Sendable {
    private let lock = NSLock()
    private var _requested = false

    var requested: Bool {
        lock.lock()
        defer { lock.unlock() }
        return _requested
    }

    func request() {
        lock.lock()
        defer { lock.unlock() }
        _requested = true
    }
}

final class AsyncEvent: @unchecked Sendable {
    private let lock = NSLock()
    private var continuations: [CheckedContinuation<Void, Never>] = []
    private var fired = false

    func signal() {
        lock.lock()
        
        let waiters = continuations
        continuations.removeAll()
        fired = true
        
        lock.unlock()
        
        for c in waiters { c.resume() }
    }

    func wait() async {
        await withCheckedContinuation { c in
            lock.lock()
            if fired {
                lock.unlock()
                c.resume()
            } else {
                continuations.append(c)
                lock.unlock()
            }
        }
    }
}