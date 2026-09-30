# Contributing to blewred

Thank you for your interest in contributing to **`blewred`**! 

`blewred` is a high-performance, real-time preemptive protection suite for content creators and live streamers. It shields live broadcasts from unintended NSFW imagery and prohibited words (stopwords) before they reach the audience canvas.

This document outlines the architecture, coding standards, UI conventions, and contribution workflows for anyone who wishes to contribute to the project.

---

## 1. Core Principles & Philosophy

### Strict Lowercase Project Name
- The name of this project is strictly lowercase: **`blewred`** (never capitalized as `Blewred`, `BLEWRED`, or `BlewRed` in code, documentation, or user interfaces).

### Local & Privacy-Preserving
- `blewred` runs **100% locally on the streamer's PC**. No audio, screenshots, frames, or telemetry are ever sent to external cloud APIs or remote servers.

### Zero-Latency Architecture
- Broadcast streams cannot tolerate lag. Every frame analysis and censorship decision must execute in under 15 milliseconds.

---

## 2. System Architecture

Understanding the architectural foundations of `blewred` ensures your contributions integrate smoothly:

```
┌────────────────────────────────────────────────────────────────────────┐
│                              blewred                                   │
├───────────────────────────────────┬────────────────────────────────────┤
│         Frontend (UI)             │         Backend (Core)             │
│  • Tauri v2 Webview               │  • Rust 1.77+ (Tokio Async)        │
│  • Vanilla JS + Bootstrap v3 Dark │  • DirectML (DirectX 12 GPU)       │
│  • Symmetrical i18n (RU & EN)     │  • Native Windows.Media.Ocr (WinRT)│
│  • Zero HTML title attributes     │  • OBS WebSocket v5 & SHM Filter   │
└───────────────────────────────────┴────────────────────────────────────┘
```

### DirectML vs CUDA
- `blewred` uses **ONNX Runtime (`ort`) with the `directml` execution provider** (`ort = { version = "2.0.0-rc.13", features = ["directml"] }`).
- **DirectML compiles HLSL compute shaders on DirectX 12 Compute (Direct3D 12).**
- **No CUDA Toolkit or cuDNN required:** DirectML runs natively across all modern GPUs (NVIDIA GeForce, AMD Radeon, and Intel Arc / Iris Xe) using standard Windows display drivers.
- **Do not introduce CUDA dependencies or terminology.** The system relies exclusively on DirectML and DirectX 12.

### Dual-Stage Neural Cascade
1. **Stage 1 (ViT Screener):** Falconsai Vision Transformer (`vit_nsfw.onnx`, 224×224). Runs lightweight, ultra-fast binary classification (NSFW vs Normal) in ~8–12 ms.
2. **Stage 2 (NudeNet 640m Localizer):** YOLOv8-based model (`640m.onnx`, 640×640). Runs only when Stage 1 triggers or during active tracking, producing precise bounding boxes for targeted blurring.
3. **ZeroMiss Spatial Tracker:** Prevents flicker by smoothing bounding box movement across consecutive frames.

### Native Windows OCR
- Stopword detection uses native `Windows.Media.Ocr` via Windows Runtime (`windows` crate).
- Requires zero external downloads, models, or language packs on Windows 10/11.

### HTML5 WebExtension Lookahead
- A lightweight Chromium extension syncs playback position and timecodes from web video players (YouTube, Twitch, VK, etc.) with a 5-second horizon.
- It **does not intercept audio or subtitles**; it synchronizes timecodes so the engine can warn the streamer or schedule censorship ahead of time.

---

## 3. Code & UI Conventions

All contributors must adhere to the following project guidelines:

### Language Guidelines
- **Internal Code:** Exclusively **English**. All Rust code, JavaScript functions, variable names, comments, docstrings (`///`), logs, commit messages, and PR descriptions must be written in English.
- **User-Facing UI:** Symmetrical **bilingual localization** (Russian and English).
  - Every UI string must be present in both the `ru` and `en` dictionaries in `ui/i18n.js` with identical keys.
  - HTML elements must be tagged with `data-i18n`, `data-i18n-placeholder`, or `data-i18n-aria`.
  - Universal technical standards are exempt from translation (`GPU`, `FPS`, `OBS`, `CUDA`, `DirectML`, `ON`, `OFF`, `F8`, `F9`, `blewred`).

### Strict Prohibition of HTML `title` Attributes
- **Zero HTML `title` attributes are allowed anywhere in the codebase.**
- Native browser hover tooltips create disruptive visual clutter during live broadcasts.
- Do not use `title="..."` in HTML markup or dynamic JavaScript element creation (`elem.title = ...` or `elem.setAttribute('title', ...)`).
- Use `aria-label`, visible badge labels, or dedicated status containers instead.

### UI Aesthetic: Bootstrap v3 Dark Edition
- The UI follows a high-density, technical dark theme inspired by Bootstrap v3 Dark Edition (`ui/style.css`, `ui/bootstrap3-dark.css`).
- Use subtle linear gradients, dark panels (`panel-default`, `panel-heading`), crisp borders, and compact typography (Inter / Roboto).

---

## 4. Development Setup

### Prerequisites
- **Operating System:** Windows 10 or Windows 11 (64-bit).
- **Rust Toolchain:** Rust 1.77+ with MSVC toolchain:
  ```powershell
  rustup default stable-x86_64-pc-windows-msvc
  ```
- **C++ Build Tools:** Visual Studio 2022 Build Tools with the **Desktop development with C++** workload and the Windows 10/11 SDK installed.
- **Node.js:** v18.0+ (used for JavaScript syntax verification and linting).
- **Git:** Git for Windows.

### Getting Started

1. **Clone the repository:**
   ```bash
   git clone https://github.com/blewred/blewred.git
   cd blewred
   ```

2. **Run in development mode:**
   ```cmd
   run.bat
   ```
   *Alternatively, if Tauri CLI is installed globally:*
   ```bash
   cargo tauri dev
   ```

3. **Compile-check the Rust backend:**
   ```bash
   cargo check --manifest-path src-tauri/Cargo.toml
   ```

4. **Run all unit and integration tests:**
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml
   ```

5. **Validate JavaScript syntax:**
   ```bash
   node -c ui/app.js
   node -c ui/i18n.js
   ```

6. **Verify zero HTML `title` attributes:**
   ```powershell
   Select-String -Path "ui\index.html", "ui\app.js" -Pattern '\btitle\s*='
   ```
   *(This must return zero matches).*

---

## 5. Development Workflow

### Branching Strategy
- Branch from `main`.
- Name your branch descriptively:
  - `feature/your-feature-name`
  - `fix/bug-description`
  - `docs/documentation-update`

### Commit Message Guidelines
We follow standard Conventional Commits:
- `feat: add monitor selection for HUD alerts`
- `fix: eliminate duplicate OCR test handler in app.js`
- `docs: update streamer quickstart guide with DirectML requirements`
- `refactor: extract make_incident_json helper in backend`

All commit messages must be in English.

### Documentation Maintenance (ReadMeKeeper)
Whenever technical changes, architecture modifications, or configuration adjustments are made:
- Update `README.md` (English) and `README.ru.md` (Russian).
- Update relevant guides in `docs/` or `docs/ru/`.

---

## 6. Pre-Flight Pull Request Checklist

Before submitting a Pull Request, verify that:

- [ ] Project name is strictly lowercase `blewred`.
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` finishes with 0 errors and 0 warnings.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` passes all unit and integration tests.
- [ ] `node -c ui/app.js` and `node -c ui/i18n.js` validate without syntax errors.
- [ ] No duplicate event listeners, duplicate functions, or dead code blocks were introduced.
- [ ] No HTML `title` attributes exist in markup or scripts.
- [ ] All new user-facing strings are localized symmetrically in both `ru` and `en` in `ui/i18n.js`.
- [ ] All internal code, comments, docstrings, and commits are exclusively in English.
- [ ] Hardware acceleration references state DirectML (DirectX 12) rather than CUDA.

---

## 7. Reporting Issues

If you find a bug or have a suggestion:
1. Check existing issues and discussions to avoid duplicates.
2. Provide a clear, descriptive title.
3. Include your environment details (Windows version, GPU model, driver version).
4. Outline reproduction steps and attach relevant logs if available.

Thank you for helping make `blewred` the best real-time stream protection suite!
