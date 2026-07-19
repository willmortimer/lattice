// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LatticeVoiceBridge",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "LatticeVoiceBridge",
            type: .dynamic,
            targets: ["LatticeVoiceBridge"]
        ),
        .executable(
            name: "lattice-voice-bridge-smoke",
            targets: ["LatticeVoiceBridgeSmoke"]
        )
    ],
    dependencies: [
        .package(
            url: "https://github.com/FluidInference/FluidAudio.git",
            exact: "0.15.5"
        )
    ],
    targets: [
        .target(
            name: "LatticeVoiceBridgeC",
            path: "Sources/LatticeVoiceBridgeC",
            publicHeadersPath: "include"
        ),
        .target(
            name: "LatticeVoiceBridge",
            dependencies: [
                "LatticeVoiceBridgeC",
                .product(name: "FluidAudio", package: "FluidAudio")
            ],
            path: "Sources/LatticeVoiceBridge"
        ),
        .executableTarget(
            name: "LatticeVoiceBridgeSmoke",
            dependencies: ["LatticeVoiceBridge", "LatticeVoiceBridgeC"],
            path: "Sources/LatticeVoiceBridgeSmoke"
        )
    ]
)
