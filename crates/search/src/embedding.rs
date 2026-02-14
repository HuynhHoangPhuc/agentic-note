use agentic_note_core::error::{AgenticError, Result};
use ort::session::Session;
use rusqlite::Connection;
use std::path::Path;

/// Embedding index backed by ort (ONNX Runtime) and SQLite for vector storage.
/// Uses brute-force cosine similarity — suitable for <10k notes.
pub struct EmbeddingIndex {
    session: Session,
}

impl EmbeddingIndex {
    /// Load ONNX session from model path and initialize SQLite embedding table.
    pub fn open(db: &Connection, model_path: &Path) -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| AgenticError::Embedding(format!("session builder: {e}")))?
            .commit_from_file(model_path)
            .map_err(|e| AgenticError::Embedding(format!("load model: {e}")))?;

        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS note_embeddings (
                note_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL
            )",
        )
        .map_err(|e| AgenticError::Embedding(format!("create table: {e}")))?;

        Ok(Self { session })
    }

    /// Generate a 384-dim embedding for the given text.
    /// Uses simplified word-level tokenization (CLS=101, SEP=102, pad=0).
    pub fn generate_embedding(&mut self, text: &str) -> Result<Vec<f32>> {
        // Simple tokenization: map words to ascending IDs with CLS/SEP framing
        let word_ids: Vec<i64> = std::iter::once(101i64) // CLS token
            .chain(
                text.split_whitespace()
                    .take(254)
                    .enumerate()
                    .map(|(i, _)| (i + 1000) as i64),
            )
            .chain(std::iter::once(102i64)) // SEP token
            .collect();

        if word_ids.len() <= 2 {
            return Ok(vec![0.0f32; 384]);
        }

        let seq_len = word_ids.len();
        let attention: Vec<i64> = vec![1i64; seq_len];
        let type_ids: Vec<i64> = vec![0i64; seq_len];

        let input_ids = ort::value::Value::from_array(([1usize, seq_len], word_ids))
            .map_err(|e| AgenticError::Embedding(format!("input tensor: {e}")))?;
        let attn_mask = ort::value::Value::from_array(([1usize, seq_len], attention))
            .map_err(|e| AgenticError::Embedding(format!("attn tensor: {e}")))?;
        let token_types = ort::value::Value::from_array(([1usize, seq_len], type_ids))
            .map_err(|e| AgenticError::Embedding(format!("type tensor: {e}")))?;

        let outputs = self
            .session
            .run(ort::inputs![input_ids, attn_mask, token_types])
            .map_err(|e| AgenticError::Embedding(format!("inference: {e}")))?;

        // Get first output tensor
        let output_value = outputs
            .values()
            .next()
            .ok_or_else(|| AgenticError::Embedding("no output tensor".into()))?;

        let (shape, data) = output_value
            .try_extract_tensor::<f32>()
            .map_err(|e| AgenticError::Embedding(format!("extract tensor: {e}")))?;

        // Mean pooling across sequence dimension
        let dim = if shape.len() == 3 {
            shape[2] as usize
        } else {
            384
        };
        let tokens = if shape.len() == 3 {
            shape[1] as usize
        } else {
            1
        };

        let mut pooled = vec![0.0f32; dim];
        for t in 0..tokens {
            for d in 0..dim {
                pooled[d] += data[t * dim + d];
            }
        }
        for v in &mut pooled {
            *v /= tokens as f32;
        }

        // L2 normalize
        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut pooled {
                *v /= norm;
            }
        }

        Ok(pooled)
    }

    /// Index a note's embedding in SQLite.
    pub fn index_note(&mut self, db: &Connection, note_id: &str, text: &str) -> Result<()> {
        let embedding = self.generate_embedding(text)?;
        let bytes = embedding_to_bytes(&embedding);
        db.execute(
            "INSERT OR REPLACE INTO note_embeddings (note_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![note_id, bytes],
        )
        .map_err(|e| AgenticError::Embedding(format!("insert: {e}")))?;
        Ok(())
    }

    /// Remove a note's embedding.
    pub fn remove_note(db: &Connection, note_id: &str) -> Result<()> {
        db.execute(
            "DELETE FROM note_embeddings WHERE note_id = ?1",
            rusqlite::params![note_id],
        )
        .map_err(|e| AgenticError::Embedding(format!("delete: {e}")))?;
        Ok(())
    }

    /// Brute-force KNN search via cosine similarity.
    pub fn search(
        &mut self,
        db: &Connection,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        let query_vec = self.generate_embedding(query)?;
        let mut stmt = db
            .prepare("SELECT note_id, embedding FROM note_embeddings")
            .map_err(|e| AgenticError::Embedding(format!("query: {e}")))?;

        let mut results: Vec<(String, f32)> = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((id, blob))
            })
            .map_err(|e| AgenticError::Embedding(format!("iterate: {e}")))?
            .filter_map(|r| r.ok())
            .map(|(id, blob)| {
                let emb = bytes_to_embedding(&blob);
                let score = cosine_similarity(&query_vec, &emb);
                (id, score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    }
}

fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}
