// swift-tools-version: 5.9
// SPDX-License-Identifier: Apache-2.0

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