# Research: Forward Secrecy / Double Ratchet & GitHub Actions CI/CD for Rust
Date: 2026-02-18 | Project: agentic-note (Cargo workspace, 8 crates)

---

## Topic 1: Forward Secrecy / Double Ratchet in Rust

### Current State
Static X25519 DH (chacha20poly1305 + x25519-dalek). No ephemeral key rotation = no forward secrecy.

### Crate Options

| Crate | Status | Notes |
|---|---|---|
| [`double-ratchet`](https://crates.io/crates/double-ratchet) | Unmaintained | Original Trevor Perrin/Moxie spec impl |
| [`ksi-double-ratchet`](https://docs.rs/ksi-double-ratchet/) | Active | Drop-in replacement for above; modern deps + bug fixes |
| [`double-ratchet-signal`](https://crates.io/crates/double-ratchet-signal) | Active | Signal-protocol flavor |
| [`light-double-ratchet`](https://docs.rs/light-double-ratchet/) | Active | Minimal; good for embedded/CLI |
| [`vodozemac`](https://matrix-org.github.io/vodozemac/vodozemac/) | Active, audited | Matrix/Olm variant of Double Ratchet; security-audited by Least Authority |

### Recommendation
**`ksi-double-ratchet`** for P2P sync - lightest surface, Signal-spec compliant, maintained.
**`vodozemac`** only if Matrix ecosystem compatibility is needed (heavier dep).

### Protocol Flow (P2P Sync)
1. Initial handshake: X3DH (Extended Triple DH) establishes shared root key using ephemeral + identity keys
2. Double Ratchet takes over: DH ratchet per message + symmetric chain ratchet
3. Each message uses a derived one-time key → compromise of one key doesn't reveal past/future keys

### Backward Compat Strategy (static → ephemeral migration)
- Add version byte to encrypted envelope (0x01 = legacy static, 0x02 = double-ratchet)
- Legacy peers: fall back to static X25519 (no forward secrecy, but still works)
- New peers: negotiate DR on first sync handshake via session state stored in local DB
- Migration: phased; existing encrypted data stays decryptable under old scheme

### Complexity vs Security Tradeoff
- **DR adds**: session state persistence (30-100 bytes per peer), ratchet state DB table
- **DR gives**: per-message forward secrecy, break-in recovery (future secrecy)
- **Verdict**: Worth it for P2P sync; complexity is manageable with `ksi-double-ratchet`'s abstraction

---

## Topic 2: GitHub Actions CI/CD for Rust Cargo Workspace

### Caching Strategy (most impactful optimization)
**Use [`Swatinem/rust-cache@v2`](https://github.com/Swatinem/rust-cache)** - caches `~/.cargo` registry + `./target` deps.

```yaml
- uses: Swatinem/rust-cache@v2
  with:
    workspaces: ". -> target"          # workspace root
    cache-on-failure: true
    shared-key: "rust-${{ matrix.feature-set }}"
```

For large workspaces, `sccache` (via `mozilla-actions/sccache-action`) wraps rustc and uses GHA cache backend - better cache hit rates across jobs.

### Feature Flag Test Matrix
```yaml
strategy:
  matrix:
    feature-set:
      - ""                          # default features only
      - "--no-default-features"
      - "--features embeddings"
      - "--features postgres"
      - "--features prometheus"
      - "--all-features"
    os: [ubuntu-latest, macos-latest]
```
Run per crate or use `cargo test --workspace --features ${{ matrix.feature-set }}`.

### Full CI Workflow Structure
```
Jobs (parallel):
├── check       → cargo check --workspace --all-features
├── fmt         → cargo fmt --all --check
├── clippy      → cargo clippy --workspace --all-features -- -D warnings
├── audit       → cargo audit (rustsec advisory db)
├── test-matrix → matrix of feature sets above
└── release     → (on tag) cross-compile + upload artifacts
```

### Binary Release Automation (cross-compile)
**Tool: [`houseabsolute/actions-rust-cross`](https://github.com/houseabsolute/actions-rust-cross)** wraps `cross` tool.

Targets to cover:
- `x86_64-unknown-linux-gnu` (Linux native runner, no cross needed)
- `aarch64-unknown-linux-gnu` (cross via Docker)
- `x86_64-apple-darwin` (macOS runner)
- `aarch64-apple-darwin` (macOS runner, Apple Silicon)
- `x86_64-pc-windows-msvc` (Windows runner)

```yaml
- uses: houseabsolute/actions-rust-cross@v0
  with:
    command: build
    target: ${{ matrix.target }}
    args: "--release --bin agentic-note"
```

Artifacts: Linux/macOS → `.tar.gz`, Windows → `.zip`. Named `{bin}-{version}-{target}`.

Trigger: `on: push: tags: ['v*']`. Upload via `softprops/action-gh-release`.

### Recommended Minimal CI (start here, expand as needed)
```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-features -- -D warnings
      - run: cargo test --workspace
      - run: cargo audit
```
Add matrix + release job when workspace stabilizes.

---

## Unresolved Questions
1. Does agentic-note P2P sync use persistent sessions (need ratchet state DB) or one-shot encryption?
2. Which 8 crates need feature flag testing vs. always-on? Avoids matrix explosion.
3. macOS cross-compile to Linux: GitHub's macOS runners can't easily cross-compile to Linux; need Linux runner + cross for that direction.
4. `cargo audit` needs `cargo-audit` installed — use `cargo install` step or pre-built action?

## Sources
- [vodozemac](https://matrix-org.github.io/vodozemac/vodozemac/index.html)
- [ksi-double-ratchet](https://docs.rs/ksi-double-ratchet/latest/ksi_double_ratchet/)
- [double-ratchet-signal](https://crates.io/crates/double-ratchet-signal)
- [light-double-ratchet](https://docs.rs/light-double-ratchet/latest/light_double_ratchet/)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
- [actions-rust-cross](https://github.com/houseabsolute/actions-rust-cross)
- [sccache in GitHub Actions](https://depot.dev/blog/sccache-in-github-actions)
- [Cross-platform Rust pipeline 2025](https://ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/)
