  # Build (CPU)
  cd simse-engine && cargo build --release

  # Build with GPU
  cargo build --release --features cuda   # or metal, mkl

  # Build for WASM
  cargo build --release --target wasm32-wasip1

  # Build with embedded weights
  SIMSE_GEN_MODEL_PATH=./model.gguf cargo build --release --features embed-weights


Register in ~/.simse/acp.json:
  {
    "servers": [{
      "name": "simse-engine",
      "command": "simse-engine",
      "args": ["--model", "bartowski/Llama-3.2-3B-Instruct-GGUF",
               "--model-file", "Llama-3.2-3B-Instruct-Q4_K_M.gguf",
               "--embedding-model", "nomic-ai/nomic-embed-text-v1.5"]
    }]
  }
