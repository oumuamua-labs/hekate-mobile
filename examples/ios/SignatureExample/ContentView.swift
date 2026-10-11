// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import SwiftUI

struct ContentView: View {
    @StateObject private var model = RunModel()

    var body: some View {
        NavigationStack {
            List {
                Section("What the proof states") {
                    Picker("Statement", selection: $model.statement) {
                        ForEach(Statement.allCases) { statement in
                            Text(statement.rawValue).tag(statement)
                        }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("statement")
                    .disabled(model.isRunning)

                    Text(model.statement.claim)
                    LabeledContent("Public", value: model.statement.publicValues)
                    LabeledContent("Hidden", value: model.statement.hiddenValues)
                }

                Section {
                    controls
                }

                if case .finished(let report) = model.phase {
                    Section("Result") {
                        ForEach(report.rows) { row in
                            LabeledContent(row.label, value: row.value)
                        }

                        ShareLink("Share the report", item: report.text)
                    }
                }
            }
            .navigationTitle("Hekate signatures")
        }
    }

    @ViewBuilder
    private var controls: some View {
        switch model.phase {
        case .idle:
            runButton
        case .running(let step):
            HStack {
                ProgressView()
                Text(step)
            }

            Button("Cancel", role: .cancel, action: model.cancel)
                .accessibilityIdentifier("cancel")
        case .finished(let report):
            Label(report.verdict, systemImage: "checkmark.seal")
                .accessibilityIdentifier("verdict")

            runButton
        case .cancelled:
            Text("Cancelled")
                .accessibilityIdentifier("cancelled")

            runButton
        case .failed(let message):
            Label(message, systemImage: "xmark.octagon")
                .foregroundStyle(.red)
                .accessibilityIdentifier("failure")

            runButton
        }
    }

    @ViewBuilder
    private var runButton: some View {
        if let reason = model.statement.unavailableReason {
            Text(reason)
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("unavailable")
        } else {
            Button("Sign, prove and verify", action: model.start)
                .accessibilityIdentifier("run")
        }
    }
}