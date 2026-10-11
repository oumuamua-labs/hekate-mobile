// swift-tools-version: 5.9
// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import PackageDescription

let package = Package(
    name: "hekate-swift",
    platforms: [
        .iOS(.v16),
        .macOS(.v13),
    ],
    products: [
        .library(name: "Hekate", targets: ["Hekate"]),
    ],
    targets: [
        .target(name: "Hekate", path: "swift/Sources/Hekate"),
        .testTarget(
            name: "HekateTests",
            dependencies: ["Hekate"],
            path: "swift/Tests/HekateTests"
        ),
    ]
)