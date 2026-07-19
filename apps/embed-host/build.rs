fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    prost_build::Config::new()
        .protoc_executable(protoc)
        .compile_protos(&["proto/embed_host.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/embed_host.proto");
    Ok(())
}
