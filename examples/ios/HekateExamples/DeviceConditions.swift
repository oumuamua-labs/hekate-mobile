// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import UIKit

struct DeviceConditions: Sendable {
    let model: String
    let isSimulator: Bool
    let system: String
    let lowPowerMode: Bool
    let power: String
    let thermalState: String

    @MainActor
    static func current() -> DeviceConditions {
        let device = UIDevice.current
        let process = ProcessInfo.processInfo

        device.isBatteryMonitoringEnabled = true

        return DeviceConditions(
            model: modelIdentifier(),
            isSimulator: isSimulator,
            system: "\(device.systemName) \(process.operatingSystemVersionString)",
            lowPowerMode: process.isLowPowerModeEnabled,
            power: describe(device.batteryState),
            thermalState: currentThermalState()
        )
    }

    static func currentThermalState() -> String {
        let state = ProcessInfo.processInfo.thermalState

        switch state {
        case .nominal: return "nominal"
        case .fair: return "fair"
        case .serious: return "serious"
        case .critical: return "critical"
        @unknown default: return "unknown (\(state.rawValue))"
        }
    }

    #if targetEnvironment(simulator)
    private static let isSimulator = true

    private static func modelIdentifier() -> String {
        ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] ?? "unknown"
    }
    #else
    private static let isSimulator = false

    private static func modelIdentifier() -> String {
        var system = utsname()

        guard uname(&system) == 0 else {
            return "unknown"
        }

        return withUnsafeBytes(of: &system.machine) { bytes in
            String(decoding: bytes.prefix { $0 != 0 }, as: UTF8.self)
        }
    }
    #endif

    private static func describe(_ state: UIDevice.BatteryState) -> String {
        switch state {
        case .unplugged: return "on battery"
        case .charging: return "charging"
        case .full: return "plugged in, full"
        case .unknown: return "unknown"
        @unknown default: return "unknown (\(state.rawValue))"
        }
    }
}