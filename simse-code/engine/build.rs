fn main() {
    #[cfg(feature = "embed-weights")]
    {
        if let Ok(path) = std::env::var("SIMSE_GEN_MODEL_PATH") {
            println!("cargo:rustc-env=SIMSE_GEN_MODEL_PATH={path}");
            println!("cargo:rerun-if-env-changed=SIMSE_GEN_MODEL_PATH");
        }
        if let Ok(path) = std::env::var("SIMSE_EMBED_MODEL_PATH") {
            println!("cargo:rustc-env=SIMSE_EMBED_MODEL_PATH={path}");
            println!("cargo:rerun-if-env-changed=SIMSE_EMBED_MODEL_PATH");
        }
    }
}
