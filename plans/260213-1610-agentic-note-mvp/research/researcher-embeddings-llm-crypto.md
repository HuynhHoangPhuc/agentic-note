# Embeddings, LLM Providers & Crypto in Rust
**Date:** 2026-02-13 | Knowledge cutoff: Jan 2025

---

## 1. sqlite-vec in Rust

**Verdict:** `sqlite-vec` has a Rust crate (`sqlite-vec` v0.1.x) that bundles the extension and loads it via rusqlite. Stable enough for MVP.

**Cargo.toml:**
```toml
sqlite-vec = "0.1"
rusqlite = { version = "0.31", features = ["bundled"] }
```

**Usage pattern:**
```rust
use rusqlite::Connection;

let db = Connection::open("notes.db")?;
sqlite_vec::sqlite3_auto_extension(); // register extension

// Schema
db.execute_batch("
    CREATE VIRTUAL TABLE IF NOT EXISTS vec_notes USING vec0(
        note_id INTEGER PRIMARY KEY,
        embedding FLOAT[384]   -- all-MiniLM-L6 output dim
    );
")?;

// Insert
db.execute(
    "INSERT INTO vec_notes (note_id, embedding) VALUES (?1, ?2)",
    (id, sqlite_vec::as_bytes(&embedding_vec)),  // serialize [f32;384] → bytes
)?;

// KNN query
let results: Vec<(i64, f32)> = db.prepare(
    "SELECT note_id, distance FROM vec_notes
     WHERE embedding MATCH ?1 AND k = 10
     ORDER BY distance"
)?
.query_map([sqlite_vec::as_bytes(&query_vec)], |r| {
    Ok((r.get::<_, i64>(0)?, r.get::<_, f32>(1)?))
})?
.collect::<Result<_, _>>()?;
```

**Key API:** `sqlite_vec::as_bytes(&[f32])` serializes f32 slice to the wire format. Use `sqlite_vec::sqlite3_auto_extension()` once at startup.

**Fallback if sqlite-vec unavailable:** Store embedding as `BLOB` (raw f32 bytes), compute cosine similarity in Rust after fetching candidates. For <10k notes, a full scan with SIMD cosine is fast enough.

```rust
// Manual cosine similarity fallback
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}
```

---

## 2. all-MiniLM-L6-v2 in Rust

**Options ranked by simplicity:**

| Crate | Simplicity | Binary size | Notes |
|-------|-----------|-------------|-------|
| `ort` (ONNX Runtime) | High | +15-30MB (native lib) | Best choice — no ML knowledge needed |
| `candle` (HF) | Medium | +5MB (pure Rust) | Requires implementing tokenizer setup |
| `rust-bert` | Low | +500MB (tch/libtorch) | Overkill for embeddings only |

**Recommended: `ort` + ONNX model file**

Model: download `sentence-transformers/all-MiniLM-L6-v2` ONNX from HuggingFace.

```toml
ort = { version = "2.0", features = ["load-dynamic"] }
tokenizers = "0.19"
```

```rust
use ort::{Environment, Session, Value};
use tokenizers::Tokenizer;

struct EmbeddingModel {
    session: Session,
    tokenizer: Tokenizer,
}

impl EmbeddingModel {
    fn new(model_path: &str, tokenizer_path: &str) -> anyhow::Result<Self> {
        let env = Environment::builder().build()?.into_arc();
        let session = Session::builder(&env)?
            .with_model_from_file(model_path)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path).unwrap();
        Ok(Self { session, tokenizer })
    }

    fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true).unwrap();
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();

        let input_ids = Value::from_array(([1, ids.len()], ids.as_slice()))?;
        let attention_mask = Value::from_array(([1, mask.len()], mask.as_slice()))?;

        let outputs = self.session.run(vec![input_ids, attention_mask])?;
        let embeddings = outputs[0].try_extract::<f32>()?;
        // Mean pool + L2 normalize → 384-dim vector
        let view = embeddings.view();
        let pooled = mean_pool(&view);   // implement mean across token dim
        Ok(l2_normalize(pooled))
    }
}
```

**Model files to ship:** `model.onnx` (~23MB), `tokenizer.json` (~250KB). Store in `~/.local/share/agentic-note/models/`.

**candle alternative** (pure Rust, no native dep):
```toml
candle-core = "0.7"
candle-nn = "0.7"
candle-transformers = "0.7"
hf-hub = "0.3"
tokenizers = "0.19"
```
More code but no dynamic library dependency — better for distribution.

---

## 3. LLM Provider Abstraction in Rust

**Pattern: trait object + registry map**

```rust
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> anyhow::Result<String>;
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;  // optional default impl
}

#[derive(Clone, Debug)]
pub struct Message { pub role: String, pub content: String }

#[derive(Clone, Debug, Default)]
pub struct ChatOpts { pub model: Option<String>, pub temperature: Option<f32> }
```

**Registry:**
```rust
use std::collections::HashMap;
use std::sync::Arc;

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    active: String,
}

impl ProviderRegistry {
    pub fn register(&mut self, p: Arc<dyn LlmProvider>) {
        self.providers.insert(p.name().to_string(), p);
    }
    pub fn get(&self, name: &str) -> Option<&Arc<dyn LlmProvider>> {
        self.providers.get(name)
    }
    pub fn active(&self) -> &Arc<dyn LlmProvider> {
        self.providers.get(&self.active).expect("active provider not registered")
    }
}
```

**HTTP client pattern (reqwest):**
```toml
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
```

**OpenAI provider (minimal):**
```rust
pub struct OpenAiProvider { client: reqwest::Client, api_key: String, base_url: String }

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str { "openai" }
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": opts.model.as_deref().unwrap_or("gpt-4o-mini"),
            "messages": messages
        });
        let res: Value = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await?.json().await?;
        Ok(res["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
    }
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> { todo!() }
}
```

**Ollama:** same trait, `base_url = "http://localhost:11434"`, endpoint `/api/chat`, adjust JSON shape.
**Anthropic:** same trait, different auth header (`x-api-key`), endpoint `/v1/messages`.
**Gemini:** same trait, endpoint `https://generativelanguage.googleapis.com/v1beta/...`.

**Key insight:** Ollama's API mirrors OpenAI's `/chat/completions` — `OpenAiProvider` with `base_url = "http://localhost:11434/v1"` works for Ollama with no extra code.

---

## 4. Ed25519 in Rust

**Recommended crate:** `ed25519-dalek` v2.x — maintained by Dalek Cryptography, audited.

```toml
ed25519-dalek = { version = "2", features = ["rand_core"] }
rand = "0.8"
```

```rust
use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Verifier, Signature};
use rand::rngs::OsRng;

// Generate keypair
let signing_key = SigningKey::generate(&mut OsRng);
let verifying_key: VerifyingKey = signing_key.verifying_key();

// Serialize keys
let priv_bytes: [u8; 32] = signing_key.to_bytes();
let pub_bytes: [u8; 32] = verifying_key.to_bytes();

// Deserialize
let sk = SigningKey::from_bytes(&priv_bytes);
let vk = VerifyingKey::from_bytes(&pub_bytes)?;

// Sign
let message = b"note content hash";
let sig: Signature = signing_key.sign(message);
let sig_bytes = sig.to_bytes();  // [u8; 64]

// Verify
vk.verify(message, &sig)?;

// Store as base64 or hex
let pub_b64 = base64::engine::general_purpose::STANDARD.encode(&pub_bytes);
```

**Storage pattern:** store `signing_key.to_bytes()` encrypted in OS keychain or `~/.config/agentic-note/identity.key` (0600 perms). Public key as plain text or in SQLite.

**Alternative:** `ring` crate — simpler API but less flexible. `ed25519-dalek` preferred for explicit key management.

---

## Summary

| Concern | Recommended | Notes |
|---------|-------------|-------|
| Vector search | `sqlite-vec` | KNN built-in; fallback: manual cosine |
| Embeddings | `ort` + ONNX | Simplest; model ~23MB; `candle` if no native dep |
| LLM abstraction | trait + reqwest | Ollama reuses OpenAI provider |
| Ed25519 | `ed25519-dalek` v2 | Audited, standard |

---

## Unresolved Questions

1. `sqlite-vec` Rust crate version stability — v0.1.x API may change; pin exact version.
2. ONNX model distribution — bundle in binary vs download on first run? (first-run download preferred for binary size)
3. Embedding model choice — all-MiniLM-L6-v2 (384-dim, fast) vs all-mpnet-base-v2 (768-dim, better quality)?
4. Does Anthropic provider need streaming support for MVP? (changes trait signature significantly)
5. Key storage — OS keychain (keyring crate) vs file-based for cross-platform CLI?
