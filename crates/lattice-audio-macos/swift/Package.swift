// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LatticeAudioBridge",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "LatticeAudioBridge",
            type: .dynamic,
            targets: ["LatticeAudioBridge"]
        )
    ],
    targets: [
        .target(
            name: "LatticeAudioBridgeC",
            path: "Sources/LatticeAudioBridgeC",
            publicHeadersPath: "include"
        ),
        .target(
            name: "LatticeAudioBridge",
            dependencies: ["LatticeAudioBridgeC"],
            path: "Sources/LatticeAudioBridge"
        )
    ]
)
