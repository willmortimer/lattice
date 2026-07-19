// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VoiceM0FluidAudio",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(
            name: "voice-m0-fluidaudio",
            targets: ["VoiceM0FluidAudio"]
        )
    ],
    dependencies: [
        .package(
            url: "https://github.com/FluidInference/FluidAudio.git",
            exact: "0.15.5"
        )
    ],
    targets: [
        .executableTarget(
            name: "VoiceM0FluidAudio",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio")
            ],
            path: "Sources/VoiceM0FluidAudio"
        )
    ]
)
