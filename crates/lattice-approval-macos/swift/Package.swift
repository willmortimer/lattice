// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LatticeApprovalBridge",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "LatticeApprovalBridge",
            type: .dynamic,
            targets: ["LatticeApprovalBridge"]
        )
    ],
    targets: [
        .target(
            name: "LatticeApprovalBridge",
            path: "Sources/LatticeApprovalBridge"
        )
    ]
)
