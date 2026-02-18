use agentic_note_core::error::{AgenticError, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

const MODEL_URL: &str =
    "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
const TOKENIZER_URL: &str =
    "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";

/// Default cache directory for ONNX models.
pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("agentic-note")
        .join("models")
}

/// Ensure model files exist in cache_dir, downloading if needed.
/// Returns path to the model.onnx file.
pub fn ensure_model(cache_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)?;

    let model_path = cache_dir.join("model.onnx");
    let tokenizer_path = cache_dir.join("tokenizer.json");

    if !model_path.exists() {
        tracing::info!("downloading embedding model to {}", model_path.display());
        download_file(MODEL_URL, &model_path)?;
    }
    if !tokenizer_path.exists() {
        tracing::info!("downloading tokenizer to {}", tokenizer_path.display());
        download_file(TOKENIZER_URL, &tokenizer_path)?;
    }

    Ok(model_path)
}

/// Download a file from URL with progress bar.
fn download_file(url: &str, dest: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| AgenticError::Embedding(format!("download {url}: {e}")))?;

    let total = response.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total);
    let style = ProgressStyle::default_bar()
        .template("{msg} [{bar:40}] {bytes}/{total_bytes} ({eta})")
        .map_err(|e| AgenticError::Embedding(format!("progress style: {e}")))?
        .progress_chars("=> ");
    pb.set_style(style);
    pb.set_message(
        dest.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    );

    let bytes = response
        .bytes()
        .map_err(|e| AgenticError::Embedding(format!("read response: {e}")))?;

    pb.set_position(bytes.len() as u64);
    pb.finish_with_message("done");

    let mut file = std::fs::File::create(dest)?;
    file.write_all(&bytes)?;

    Ok(())
}

/// Compute SHA-256 hash of a file.
#[allow(dead_code)]
pub fn verify_sha256(path: &Path, expected: &str) -> Result<bool> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hex = format!("{:x}", hasher.finalize());
    Ok(hex == expected)
}
