// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LatticeCaptureBridge",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "LatticeCaptureBridge",
            type: .dynamic,
            targets: ["LatticeCaptureBridge"]
        )
    ],
    targets: [
        .target(
            name: "LatticeCaptureBridgeC",
            path: "Sources/LatticeCaptureBridgeC",
            publicHeadersPath: "include"
        ),
        .target(
            name: "LatticeCaptureBridge",
            dependencies: ["LatticeCaptureBridgeC"],
            path: "Sources/LatticeCaptureBridge"
        )
    ]
)
