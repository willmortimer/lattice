//! Embed optional channel defaults at compile time (internal DMG).

fn main() {
    // When set (e.g. by scripts/release/build-internal-channel.sh), bake a
    // default cloud URL into the binary so Finder launches work without shell env.
    if let Ok(url) = std::env::var("LATTICE_CLOUD_URL_DEFAULT") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            println!("cargo:rustc-env=LATTICE_CLOUD_URL_DEFAULT={trimmed}");
        }
    }
    println!("cargo:rerun-if-env-changed=LATTICE_CLOUD_URL_DEFAULT");
}
