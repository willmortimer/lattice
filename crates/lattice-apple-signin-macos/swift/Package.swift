// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LatticeAppleSignInBridge",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "LatticeAppleSignInBridge",
            type: .dynamic,
            targets: ["LatticeAppleSignInBridge"]
        )
    ],
    targets: [
        .target(
            name: "LatticeAppleSignInBridge",
            path: "Sources/LatticeAppleSignInBridge"
        )
    ]
)
