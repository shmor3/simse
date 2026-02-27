use std::path::PathBuf;

use anyhow::Result;

/// Source for model weights — supports runtime download, local path, or compile-time embedding.
pub enum WeightSource {
    /// Download from HuggingFace Hub at runtime (default).
    HubDownload {
        repo_id: String,
        filename: String,
        revision: Option<String>,
    },
    /// Load from a local file path.
    LocalPath(PathBuf),
    /// Embedded at compile time (requires `embed-weights` feature).
    #[cfg(feature = "embed-weights")]
    Embedded(&'static [u8]),
}

impl WeightSource {
    /// Resolve to a local file path (downloading if necessary).
    #[cfg(not(target_family = "wasm"))]
    pub fn resolve(&self) -> Result<PathBuf> {
        match self {
            Self::HubDownload {
                repo_id,
                filename,
                revision,
            } => {
                tracing::info!(repo_id, filename, "Downloading model from HuggingFace Hub");
                let api = hf_hub::api::sync::Api::new()?;
                let repo = if let Some(rev) = revision {
                    api.repo(hf_hub::Repo::with_revision(
                        repo_id.clone(),
                        hf_hub::RepoType::Model,
                        rev.clone(),
                    ))
                } else {
                    api.model(repo_id.clone())
                };
                let path = repo.get(filename)?;
                tracing::info!(path = %path.display(), "Model downloaded/cached");
                Ok(path)
            }
            Self::LocalPath(path) => {
                if !path.exists() {
                    anyhow::bail!("Model file not found: {}", path.display());
                }
                Ok(path.clone())
            }
            #[cfg(feature = "embed-weights")]
            Self::Embedded(_) => {
                anyhow::bail!(
                    "Embedded weights cannot be resolved to a path. Use resolve_bytes() instead."
                )
            }
        }
    }

    /// Resolve to a local file path — WASM variant (no network downloads).
    #[cfg(target_family = "wasm")]
    pub fn resolve(&self) -> Result<PathBuf> {
        match self {
            Self::HubDownload { repo_id, .. } => {
                anyhow::bail!(
                    "HuggingFace Hub downloads not supported in WASM. \
                     Use --model <local-path> for model: {}",
                    repo_id
                )
            }
            Self::LocalPath(path) => {
                if !path.exists() {
                    anyhow::bail!("Model file not found: {}", path.display());
                }
                Ok(path.clone())
            }
            #[cfg(feature = "embed-weights")]
            Self::Embedded(_) => {
                anyhow::bail!(
                    "Embedded weights cannot be resolved to a path. Use resolve_bytes() instead."
                )
            }
        }
    }

    /// Resolve to raw bytes (for embedded weights).
    #[cfg(feature = "embed-weights")]
    pub fn resolve_bytes(&self) -> Result<&'static [u8]> {
        match self {
            Self::Embedded(bytes) => Ok(bytes),
            _ => anyhow::bail!("resolve_bytes() only works with embedded weights"),
        }
    }
}

// ── Compile-time embedded weights ─────────────────────────────────────────

#[cfg(feature = "embed-weights")]
pub mod embedded {
    /// Text generation model weights (set via SIMSE_GEN_MODEL_PATH env var at build time).
    #[cfg(feature = "embed-weights")]
    pub static GEN_WEIGHTS: Option<&[u8]> = if cfg!(feature = "embed-weights") {
        option_env!("SIMSE_GEN_MODEL_PATH").map(|_| include_bytes!(env!("SIMSE_GEN_MODEL_PATH")) as &[u8])
    } else {
        None
    };

    /// Embedding model weights (set via SIMSE_EMBED_MODEL_PATH env var at build time).
    #[cfg(feature = "embed-weights")]
    pub static EMBED_WEIGHTS: Option<&[u8]> = if cfg!(feature = "embed-weights") {
        option_env!("SIMSE_EMBED_MODEL_PATH").map(|_| include_bytes!(env!("SIMSE_EMBED_MODEL_PATH")) as &[u8])
    } else {
        None
    };
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Determine the weight source from a model identifier string.
/// If it looks like a file path (contains / or \ and exists), use LocalPath.
/// Otherwise, treat it as a HuggingFace repo ID.
///
/// When no filename is provided, queries the HuggingFace Hub repo to find
/// a GGUF file automatically (preferring Q4_K_M quantization).
pub fn resolve_source(model_id: &str, filename: Option<&str>, revision: Option<&str>) -> WeightSource {
    let path = PathBuf::from(model_id);
    if path.exists() {
        return WeightSource::LocalPath(path);
    }

    if let Some(fname) = filename {
        return WeightSource::HubDownload {
            repo_id: model_id.to_string(),
            filename: fname.to_string(),
            revision: revision.map(String::from),
        };
    }

    // No filename specified — auto-discover GGUF files from the repo
    #[cfg(not(target_family = "wasm"))]
    {
        if let Ok(discovered) = discover_gguf_file(model_id) {
            tracing::info!(repo_id = model_id, filename = %discovered, "Auto-discovered GGUF file");
            return WeightSource::HubDownload {
                repo_id: model_id.to_string(),
                filename: discovered,
                revision: revision.map(String::from),
            };
        }
    }

    // Fallback: try model.gguf (unlikely to work but gives a clear 404 error)
    WeightSource::HubDownload {
        repo_id: model_id.to_string(),
        filename: "model.gguf".to_string(),
        revision: revision.map(String::from),
    }
}

/// Query HuggingFace Hub to find a GGUF file in the given repo.
/// Prefers Q4_K_M quantization, then any Q4 variant, then the first GGUF found.
#[cfg(not(target_family = "wasm"))]
fn discover_gguf_file(repo_id: &str) -> Result<String> {
    tracing::info!(repo_id, "Discovering GGUF files in repo");
    let api = hf_hub::api::sync::Api::new()?;
    let repo = api.model(repo_id.to_string());

    // hf-hub's info() returns repo metadata including sibling files
    let info = repo.info()?;
    let gguf_files: Vec<String> = info
        .siblings
        .iter()
        .map(|s| s.rfilename.clone())
        .filter(|name| name.ends_with(".gguf"))
        .collect();

    if gguf_files.is_empty() {
        anyhow::bail!("No GGUF files found in repo: {}", repo_id);
    }

    tracing::debug!(files = ?gguf_files, "Found GGUF files");

    // Prefer Q4_K_M (good quality/size balance)
    if let Some(f) = gguf_files.iter().find(|f| f.contains("Q4_K_M")) {
        return Ok(f.clone());
    }

    // Then any Q4 variant
    if let Some(f) = gguf_files.iter().find(|f| f.contains("Q4_")) {
        return Ok(f.clone());
    }

    // Then Q5_K_M
    if let Some(f) = gguf_files.iter().find(|f| f.contains("Q5_K_M")) {
        return Ok(f.clone());
    }

    // Then Q8
    if let Some(f) = gguf_files.iter().find(|f| f.contains("Q8_")) {
        return Ok(f.clone());
    }

    // Fall back to the first GGUF file
    Ok(gguf_files[0].clone())
}

/// Resolve a tokenizer from a model ID or local path.
#[cfg(not(target_family = "wasm"))]
pub fn resolve_tokenizer(model_id: &str) -> Result<PathBuf> {
    let path = PathBuf::from(model_id);

    // If model_id is a directory, look for tokenizer.json inside it
    if path.is_dir() {
        let tokenizer_path = path.join("tokenizer.json");
        if tokenizer_path.exists() {
            return Ok(tokenizer_path);
        }
        anyhow::bail!("No tokenizer.json found in {}", path.display());
    }

    // If it's a file path, look for tokenizer.json in the same directory
    if path.is_file() {
        if let Some(parent) = path.parent() {
            let tokenizer_path = parent.join("tokenizer.json");
            if tokenizer_path.exists() {
                return Ok(tokenizer_path);
            }
        }
    }

    // Otherwise, download from HuggingFace Hub
    // For GGUF repos, the tokenizer might be in a different repo
    let api = hf_hub::api::sync::Api::new()?;
    let repo = api.model(model_id.to_string());
    match repo.get("tokenizer.json") {
        Ok(path) => Ok(path),
        Err(_) => {
            // Some GGUF repos don't have tokenizer.json — try the base model repo
            tracing::warn!("No tokenizer.json in {}, will use embedded tokenizer from GGUF if available", model_id);
            anyhow::bail!("Could not find tokenizer.json for model: {}", model_id)
        }
    }
}

#[cfg(target_family = "wasm")]
pub fn resolve_tokenizer(model_id: &str) -> Result<PathBuf> {
    let path = PathBuf::from(model_id);
    if path.is_dir() {
        let tokenizer_path = path.join("tokenizer.json");
        if tokenizer_path.exists() {
            return Ok(tokenizer_path);
        }
    }
    if path.is_file() {
        if let Some(parent) = path.parent() {
            let tokenizer_path = parent.join("tokenizer.json");
            if tokenizer_path.exists() {
                return Ok(tokenizer_path);
            }
        }
    }
    anyhow::bail!(
        "Could not find tokenizer.json for model: {}. In WASM, provide a local path.",
        model_id
    )
}
