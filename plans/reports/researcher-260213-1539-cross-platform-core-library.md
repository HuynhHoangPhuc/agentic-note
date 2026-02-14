# Cross-Platform Core Library Research
**Date:** 2026-02-13 | **Slug:** cross-platform-core-library

---

## 1. Cross-Platform Core Library Approaches

### Rust + UniFFI
- **How it works:** Write core logic in Rust, generate bindings via `uniffi-bindgen` for Swift, Kotlin, Python, Go. UDL or proc-macro defines the interface.
- **Platforms:** iOS (Swift), Android (Kotlin/Java), macOS/Linux/Windows (native), WASM (wasm-bindgen, separate path), CLI (native binary).
- **Strengths:** Memory safety w/o GC, zero-cost abstractions, deterministic perf, no runtime. Mozilla uses it in Firefox (app-services), 1Password, Ditto, Automerge all use Rust cores.
- **FFI ergonomics:** UniFFI auto-generates boilerplate. Swift integration via XCFramework, Android via JNI/AAR. Mature as of 2024 (v0.27+).
- **Desktop:** Tauri v2 uses Rust backend + web frontend. CLI is trivial (pure Rust binary).
- **Weaknesses:** Steep learning curve, longer compile times, complex async story across FFI (UniFFI async support added in 0.25 but still maturing), WASM requires separate build target.

### Go + gomobile
- **How it works:** `gomobile bind` exports Go packages as native frameworks (`.xcframework` for iOS, `.aar` for Android). CLI is a native Go binary.
- **Strengths:** Fast compile, simple concurrency model (goroutines), excellent stdlib, easy cross-compile (`GOOS/GOARCH`).
- **Weaknesses:** ~2-10MB runtime adds to binary size, GC pauses (though GC is tunable), gomobile is only partially maintained, no WASM without JS bridge awkwardness, no first-class desktop GUI (relies on web wrappers). Go mobile support is "community maintained" as of 2024 - lower priority from Google.
- **vs Rust:** Go is simpler DX but trades performance guarantees and memory safety for convenience. For a local-first sync engine, Rust is more battle-tested.

### Kotlin Multiplatform (KMP)
- **Status:** Stable as of Kotlin 1.9.20 (Nov 2023), Compose Multiplatform UI also stable for iOS/Desktop.
- **Platforms:** Android, iOS (via Kotlin/Native → Objective-C interop), JVM/Desktop, WASM (experimental), Linux/Windows CLI via Kotlin/Native.
- **Strengths:** Share 100% of business logic, `expect/actual` for platform specifics, Ktor for networking, SQLDelight for DB, Kotlinx.coroutines. Single language across all targets.
- **Weaknesses:** iOS binary size bloat from Kotlin/Native runtime (~5-15MB), slower iOS build times, Kotlin/Native has GC (new MM since 1.7.20 but still not zero-GC), no first-class Swift interop (Obj-C bridge), KMP CLI on Linux/macOS requires Kotlin/Native (slower startup than native Go/Rust).
- **Ecosystem:** JetBrains driving adoption; used by Netflix, VMware, McDonald's. Growing but not as battle-tested as Rust for systems-level sync.

### TypeScript/Node.js
- **How it works:** Shared TS core, Electron for desktop, React Native for mobile, Node.js CLI.
- **Strengths:** Fastest DX, largest ecosystem (npm), single language full-stack, easiest onboarding.
- **Weaknesses:** Electron bundle size (100-150MB), memory overhead, no true mobile "core" sharing (React Native bridges JS to native), performance limitations for sync engines (CRDT ops, file hashing), no iOS background execution for JS runtime. Node.js requires bundling V8 for CLI.
- **Verdict:** Fine for UI-heavy apps, poor fit for local-first sync core that needs P2P, low-level file ops, and deterministic perf.

### Other Approaches Worth Noting
- **C/C++:** Maximum portability, used by SQLite/libgit2/WireGuard. No memory safety, tedious. Consider for embedding existing C libs (e.g., SQLite via bindings).
- **Swift + Swift Package Manager:** Good for Apple ecosystem but no Android story without manual JNI.
- **Dart/Flutter:** Flutter supports all platforms. Dart FFI for C interop. Flutter Desktop is stable. Good for UI but Dart lacks the systems-programming ecosystem for a sync core.

---

## 2. Desktop Frameworks

| Framework | Bundle Size | Performance | DX | Ecosystem | Notes |
|---|---|---|---|---|---|
| **Tauri v2** | ~3-15MB | Near-native (Rust backend) | Good (Rust+WebView) | Growing fast | Uses OS WebView; Rust core = shared w/ mobile |
| **Electron** | 100-150MB | Moderate (V8 + Chromium) | Excellent (TS/JS) | Massive | Overkill bundle; fine for DX-first apps |
| **Flutter Desktop** | ~20-30MB | Good (Skia/Impeller) | Good (Dart) | Growing | Same UI code on mobile; Dart FFI for C/Rust |
| **Native (SwiftUI/WinUI)** | ~5MB | Best | Complex (3 codebases) | Platform-native | Max DX cost, max perf |

**Winner for this use case:** Tauri v2. Rust core shared with CLI + mobile bindings; small bundle; OS WebView; v2 adds mobile support (iOS/Android as of 2024).

---

## 3. Mobile Considerations for Local-First Apps

### File System Access
- **iOS:** Sandboxed. App group containers allow sharing between app extensions. `Files.app` integration via `UIFileSharingEnabled`. No arbitrary filesystem access. iCloud Drive via `NSUbiquitousItemContainer`.
- **Android:** Scoped storage (API 29+). `MediaStore` for media, `SAF` (Storage Access Framework) for user-selected files. External storage requires runtime permission + SAF for arbitrary paths.

### Background Sync
- **iOS:** Very restricted. `BGTaskScheduler` (background fetch/processing tasks), max ~30s for fetch, longer for processing but OS-throttled. No persistent TCP connections in background. Push notifications (APNs) can wake app. **P2P sync in background is near-impossible on iOS without BLE or specific entitlements.**
- **Android:** More permissive. `WorkManager` for deferred/periodic work, `Foreground Service` for persistent sync (requires notification). `JobScheduler` for battery-optimized background work.

### P2P Networking on Mobile
- **iOS restrictions:** Multipeer Connectivity (BLE+WiFi Direct) - Apple's framework, works within Apple ecosystem only. Custom P2P over TCP/UDP requires background entitlements. **WebRTC** works in foreground. No raw socket access in background.
- **Android:** WiFi Direct (`WifiP2pManager`), BLE, regular TCP/UDP sockets. More permissive but battery drain is real.
- **Practical approach:** Use relay/signaling server as fallback for mobile P2P. Local sync via Bonjour/mDNS when on same WiFi (works on iOS foregrounded). libp2p has mobile support but iOS BG is the hard limit.

---

## 4. Recommendation

### Stack: **Rust Core + Tauri Desktop + UniFFI Mobile Bindings**

```
┌─────────────────────────────────────────┐
│           Rust Core Library             │
│  (sync engine, CRDT, P2P, markdown,     │
│   file ops, agent logic)                │
├──────────┬──────────┬───────────────────┤
│  CLI     │  Tauri   │  Mobile Bindings  │
│ (native  │ Desktop  │  (UniFFI →        │
│  binary) │ (WebView │  Swift/Kotlin)    │
│          │  + IPC)  │                   │
└──────────┴──────────┴───────────────────┘
```

**Rationale:**
1. **Rust core = single source of truth.** CRDT/sync engines (Automerge, Loro, diamond-types) are all in Rust. P2P libraries (libp2p) are Rust-native.
2. **UniFFI bridges Rust → Swift (iOS/macOS) + Kotlin (Android)** with auto-generated bindings. Async UniFFI (v0.25+) handles async Rust futures natively.
3. **Tauri v2** = desktop + mobile in one framework, uses the same Rust core via Tauri's IPC, avoids Electron's 150MB bundle.
4. **CLI = zero overhead**, just a Rust binary wrapping the core crate.
5. **Markdown + local-first:** Rust has `pulldown-cmark`, `comrak`, `tantivy` (full-text search). SQLite via `rusqlite`/`sqlx`. All battle-tested.
6. **Agentic features:** Call out to HTTP (reqwest) or embed ONNX runtime (ort crate) for local inference.

**Trade-offs to accept:**
- Higher initial complexity vs TS/Node.js
- iOS background sync severely limited (design around it: foreground-first, APNs wake for sync)
- UniFFI learning curve for Swift/Kotlin teams

**KMP is the #2 option** if team is Kotlin-heavy and wants to skip the Rust FFI complexity. But for a local-first P2P sync engine, Rust's ecosystem (Automerge, iroh, libp2p) is a decisive advantage.

---

## Unresolved Questions
1. Is the primary team language Rust-comfortable or Kotlin-heavy? Affects KMP vs Rust decision.
2. Are iOS background sync restrictions acceptable, or is real-time mobile sync a hard requirement?
3. WASM target needed (browser version)? Rust handles this via `wasm-bindgen` but adds build complexity.
4. Which P2P protocol? iroh (QUIC-based, from n0 team) vs libp2p vs custom. Each has different mobile support stories.
