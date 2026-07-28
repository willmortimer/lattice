//! Source for the copy_hello WASI fixture. Rebuild with:
//! `rustc --target wasm32-wasip1 -O fixtures/copy_hello_guest.rs -o fixtures/copy_hello.wasm`

use std::fs;
use std::io::Write;

fn main() {
    let input = fs::read("/input/hello.txt").expect("read /input/hello.txt");
    let mut out = fs::File::create("/output/out.txt").expect("create /output/out.txt");
    out.write_all(&input).expect("write /output/out.txt");
}
