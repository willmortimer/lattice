//! Stdio + non-zero exit WASI guest for capture tests.
//! Rebuild: see packages/kernelfs/README.md

fn main() {
    eprint!("boom-from-stderr");
    println!("hello-stdout");
    std::process::exit(7);
}
