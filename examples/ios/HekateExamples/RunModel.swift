// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import Foundation

struct HekateVersionUnavailable: LocalizedError {
    let frameworkId: String

    var errorDescription: String? {
        "the framework \(frameworkId) is not loaded or has no CFBundleShortVersionString"
    }
}

@MainActor
final class RunModel: ObservableObject {
    enum Phase {
        case idle
        case running(String)
        case finished(Report)
        case cancelled
        case failed(String)
    }

    @Published var statement: Statement = .rsa {
        didSet {
            inputSize = statement.inputSizes.first
            phase = .idle
        }
    }

    @Published var inputSize: InputSize? {
        didSet { phase = .idle }
    }

    @Published private(set) var phase: Phase = .idle

    private static let proverFrameworkId = "dev.oumuamua.hekate.hekateprovercdylib"

    private var task: Task<Void, Never>?

    var isRunning: Bool {
        switch phase {
        case .running: return true
        case .idle, .finished, .cancelled, .failed: return false
        }
    }

    func start() {
        let statement = statement
        let inputSize = inputSize

        phase = .running("Preparing")

        task = Task {
            do {
                phase = .finished(try await measure(statement, inputSize: inputSize))
            } catch ProveError.Cancelled {
                phase = .cancelled
            } catch {
                phase = .failed(error.localizedDescription)
            }
        }
    }

    func cancel() {
        task?.cancel()
    }

    private func measure(_ statement: Statement, inputSize: InputSize?) async throws -> Report {
        let hekateVersion = try Self.hekateVersion()
        let before = DeviceConditions.current()
        let footprintBefore = try Footprint.read()

        let measurement = try await Benchmark.measure(statement, inputSize: inputSize) { [weak self] run in
            self?.phase = .running("Proof \(run) of \(Benchmark.runs)")
        }

        return Report(
            statement: statement,
            inputSize: inputSize,
            before: before,
            thermalStateAfter: DeviceConditions.currentThermalState(),
            hekateVersion: hekateVersion,
            prover: proverBuild(),
            measurement: measurement,
            footprintBefore: footprintBefore,
            footprintAfter: try Footprint.read()
        )
    }

    private static func hekateVersion() throws -> String {
        let version = Bundle(identifier: proverFrameworkId)?
            .object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String

        guard let version else {
            throw HekateVersionUnavailable(frameworkId: proverFrameworkId)
        }

        return version
    }
}