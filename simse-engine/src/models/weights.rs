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

/// Resolve a tokenizer from a model ID, explicit tokenizer source, or local path.
///
/// Resolution order:
///   1. Explicit `--tokenizer` override (local path or HF repo)
///   2. tokenizer.json in the model directory (for local models)
///   3. tokenizer.json from the model's HF repo
///   4. Inferred base model repo (strip `-GGUF` suffix, try common orgs)
#[cfg(not(target_family = "wasm"))]
pub fn resolve_tokenizer(model_id: &str, tokenizer_source: Option<&str>) -> Result<PathBuf> {
    // 1. Explicit tokenizer source
    if let Some(source) = tokenizer_source {
        let source_path = PathBuf::from(source);
        // Direct path to tokenizer.json
        if source_path.is_file() {
            return Ok(source_path);
        }
        // Directory containing tokenizer.json
        if source_path.is_dir() {
            let tp = source_path.join("tokenizer.json");
            if tp.exists() {
                return Ok(tp);
            }
        }
        // Treat as HF repo ID
        tracing::info!(source, "Downloading tokenizer from explicit source");
        let api = hf_hub::api::sync::Api::new()?;
        let repo = api.model(source.to_string());
        return repo.get("tokenizer.json").map_err(|e| {
            anyhow::anyhow!("Failed to download tokenizer from '{}': {}", source, e)
        });
    }

    // 2. Local path checks
    let path = PathBuf::from(model_id);
    if path.is_dir() {
        let tp = path.join("tokenizer.json");
        if tp.exists() {
            return Ok(tp);
        }
    }
    if path.is_file() {
        if let Some(parent) = path.parent() {
            let tp = parent.join("tokenizer.json");
            if tp.exists() {
                return Ok(tp);
            }
        }
    }

    let api = hf_hub::api::sync::Api::new()?;

    // 3. Try the model's own HF repo
    {
        let repo = api.model(model_id.to_string());
        match repo.get("tokenizer.json") {
            Ok(path) => return Ok(path),
            Err(_) => {
                tracing::debug!("No tokenizer.json in {}", model_id);
            }
        }
    }

    // 4. Infer base model repo from GGUF repo naming conventions
    //    e.g., "bartowski/Llama-3.2-3B-Instruct-GGUF" → "meta-llama/Llama-3.2-3B-Instruct"
    if let Some(base_repo) = infer_base_model_repo(model_id) {
        tracing::info!(base_repo = %base_repo, "Trying inferred base model for tokenizer");
        let repo = api.model(base_repo.clone());
        match repo.get("tokenizer.json") {
            Ok(path) => {
                tracing::info!(base_repo, "Tokenizer found in base model repo");
                return Ok(path);
            }
            Err(_) => {
                tracing::debug!("No tokenizer.json in inferred base repo {}", base_repo);
            }
        }
    }

    anyhow::bail!(
        "Could not find tokenizer for model '{}'. Use --tokenizer <repo-or-path> to specify one.",
        model_id
    )
}

#[cfg(target_family = "wasm")]
pub fn resolve_tokenizer(model_id: &str, tokenizer_source: Option<&str>) -> Result<PathBuf> {
    if let Some(source) = tokenizer_source {
        let source_path = PathBuf::from(source);
        if source_path.is_file() {
            return Ok(source_path);
        }
        if source_path.is_dir() {
            let tp = source_path.join("tokenizer.json");
            if tp.exists() {
                return Ok(tp);
            }
        }
    }
    let path = PathBuf::from(model_id);
    if path.is_dir() {
        let tp = path.join("tokenizer.json");
        if tp.exists() {
            return Ok(tp);
        }
    }
    if path.is_file() {
        if let Some(parent) = path.parent() {
            let tp = parent.join("tokenizer.json");
            if tp.exists() {
                return Ok(tp);
            }
        }
    }
    anyhow::bail!(
        "Could not find tokenizer for model: {}. In WASM, provide a local path via --tokenizer.",
        model_id
    )
}

/// Infer the base (non-GGUF) model repo from a GGUF repo name.
///
/// Common patterns:
///   - `bartowski/Llama-3.2-3B-Instruct-GGUF` → try `meta-llama/Llama-3.2-3B-Instruct`
///   - `TheBloke/Llama-2-7B-Chat-GGUF` → try `meta-llama/Llama-2-7b-chat-hf`
///   - `user/ModelName-GGUF` → try `user/ModelName` first, then known orgs
#[cfg(not(target_family = "wasm"))]
fn infer_base_model_repo(model_id: &str) -> Option<String> {
    let parts: Vec<&str> = model_id.splitn(2, '/').collect();
    if parts.len() != 2 {
        return None;
    }

    let model_name = parts[1];

    // Strip -GGUF suffix (case-insensitive)
    let base_name = if model_name.ends_with("-GGUF") || model_name.ends_with("-gguf") {
        &model_name[..model_name.len() - 5]
    } else {
        return None; // Not a GGUF repo name
    };

    // Known org mappings for common model families
    let known_orgs = [
        "meta-llama",
        "mistralai",
        "google",
        "microsoft",
        "Qwen",
        "deepseek-ai",
        "HuggingFaceH4",
        "tiiuae",
        "01-ai",
    ];

    // Try the same org first (e.g., user uploaded both base and GGUF)
    let same_org = format!("{}/{}", parts[0], base_name);

    // Then try known orgs
    let api = hf_hub::api::sync::Api::new().ok()?;

    // Try same org
    if api.model(same_org.clone()).info().is_ok() {
        return Some(same_org);
    }

    // Try known orgs
    for org in &known_orgs {
        let candidate = format!("{}/{}", org, base_name);
        if api.model(candidate.clone()).info().is_ok() {
            return Some(candidate);
        }
    }

    None
}
