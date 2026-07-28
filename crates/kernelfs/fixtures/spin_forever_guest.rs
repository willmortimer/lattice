//! Infinite-loop WASI guest for epoch/cancel interrupt tests.
//! Rebuild: see packages/kernelfs/README.md

fn main() {
    loop {
        // Keep the body non-empty so fuel metering advances under wasip1.
        std::hint::spin_loop();
    }
}
