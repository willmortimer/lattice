#!/usr/bin/env swift
// benchmark.swift — on-device Core ML latency harness (research stub)
//
// Usage:
//   swift benchmark.swift /path/to/qwen3-embedding.mlpackage
//
// Requires macOS 14+ and a converted ML Program package.

import Foundation

#if canImport(CoreML)
import CoreML
#endif

let args = CommandLine.arguments
guard args.count >= 2 else {
    fputs(
        """
        error: missing mlpackage path.
        usage: swift benchmark.swift <path-to.mlpackage>
        See README.md for compute-unit targets (.all, .cpuAndGPU, .cpuAndNeuralEngine).

        """,
        stderr
    )
    exit(2)
}

let packagePath = (args[1] as NSString).expandingTildeInPath
var isDirectory: ObjCBool = false
guard FileManager.default.fileExists(atPath: packagePath, isDirectory: &isDirectory),
      isDirectory.boolValue
else {
    fputs("error: mlpackage not found: \(packagePath)\n", stderr)
    exit(3)
}

#if !canImport(CoreML)
fputs("error: CoreML framework unavailable on this platform.\n", stderr)
exit(4)
#else
fputs(
    """
    error: benchmark.swift is a research stub; latency benchmarking is not implemented yet.
    package: \(packagePath)
    Planned measurements: cold compile, warm query latency, peak memory, energy.
    Record results in RESULTS.md and compare against llama.cpp via lattice-embed-host.

    """,
    stderr
)
exit(1)
#endif
