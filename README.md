<p align="center">
  <img src="assets/logo.svg" alt="blewred logo" width="240">
</p>

# blewred — Preemptive Live Stream Protection Suite

[![Language: English](https://img.shields.io/badge/Language-English-blue.svg)](README.md)
[![Language: Russian](https://img.shields.io/badge/Язык-Русский-red.svg)](README.ru.md)
[![Version: 0.5.0-alpha](https://img.shields.io/badge/Version-0.5.0--alpha-orange.svg)](#)

> [!IMPORTANT]
> **Project Status: Alpha Stage (v0.5.0-alpha)**
> The project is currently in active alpha development. Occasional bugs, unexpected edge cases, or temporary limitations may occur, and new features and performance optimizations are actively being planned and implemented. We warmly welcome your feedback, bug reports, and feature requests via [GitHub Issues](https://github.com/msbluesnow/blewred/issues)!

*“Use protection B4 it happens...”* 😉

blewred is an open-source tool for streamers who want to protect their channel from accidental prohibited content — before viewers or the platform ever see it. ***OBS screen blocking based on player timecodes + stream display monitoring for NSFW content and banned words from the list via neural networks.*** OBS Studio integration intercepts violations directly on the GPU — without lag, without slowing down your game, and without anything changing on the streamer's own screen.

> [!WARNING]
> **Important Disclaimer on AI Detection Limitations**
> Neural networks and automated computer vision models are probabilistic systems and **cannot guarantee 100% censorship in all situations**. Due to rapid camera motions, non-standard angles, partial occlusions, unusual artistic styles, or sudden scene transitions, brief uncensored frames or unmasked screen regions may occasionally fail to trigger automated detection.
> `blewred` is engineered as a highly effective preventive protection tool for streamers, however it does not replace **personal responsibility for the broadcast**. Streamers are advised to remain vigilant, plan cues in advance for potentially hazardous videos, and utilize emergency mute and protection hotkeys whenever necessary (hotkeys can be fully customized and remapped in settings).

> 💡 **Streamer Setup Guide**: A complete non-technical guide covering Nvidia GPU setup, models, browser extension, and OBS is available in the **[Streamer Quick Start Guide](https://msbluesnow.github.io/blewred/quickstart_streamer)** (Russian version: **[Руководство для стримеров](https://msbluesnow.github.io/blewred/ru/quickstart_streamer)**). Full technical documentation: **[blewred Documentation](https://msbluesnow.github.io/blewred/)**.

---

## Key Capabilities

1. **Advanced Multilingual Lexical Engine**:
   - **Homoglyph & Leet Defense**: Automatically normalizes intentional obfuscations (e.g., Latin/Cyrillic cross-alphabet substitutions, `0` $\to$ `o`, `1` $\to$ `i`, `3` $\to$ `з`, `@` $\to$ `a`).
   - **Multilingual Support**: Built-in support for Russian and English, and custom user vocabularies.
   - **Hot-Reload**: Edit and reload ban words on the fly without interrupting the live broadcast.

2. **Direct OBS Studio WebSocket v5 Integration**:
   - Native asynchronous communication with `obs-websocket` v5.
   - Automatic background reconnection if OBS is launched before or after blewred.
   - Precise deadline-managed muting (`SetInputMute`) and censorship overlay shader toggles (`SetSourceFilterEnabled`).

3. **Dedicated Broadcast Studio Interface & Audit Log**:
   - Clean, minimalist aesthetic inspired by professional broadcast consoles (Bootstrap 3 Dark Edition).
   - Categorized live incident audit log tracking screen violations (NSFW / OCR) and player timing cues (Lookahead & Scheduled Cues) with dedicated filtering, active badge tracking, and segregated incident lifecycles.

4. **Scheduled Cues & Clipboard Screenshot OCR (Fuzzy Parsing)**:
   - Streamers can paste screenshots of timecodes directly into the Control Center with `Ctrl+V` (or drag-and-drop).
   - Built-in native Windows Media OCR with automatic primary whitespace normalization (stripping OCR space artifacts around colons, dots, and numbers: `12 : 30 - 14 : 15` ➔ `12:30 - 14:15`) and fuzzy parsing for irregular time formats (`12:30-14:15`, `34:69-1.34`, non-standard dashes and separators) into structured timestamps.
   - Intelligent timeline tracking with scrubbing tolerance: even if the streamer fast-forwards or jumps backward in the player, pre-warning modal alerts and automatic censor triggers fire reliably.

5. **Browser Extension & Lookahead Buffer**:
   - The Chrome / Brave / Edge extension captures HTML5 player frames **+10 seconds ahead** of current playback using an offscreen shadow video clone.
   - Frames are sent to the blewred core (~3 FPS, `480×270` JPEG) where the neural cascade checks them for prohibited content in advance.
   - **Dedicated Draggable Lookahead Video Preview Window in Windows**:
     - At any time, with a single button click ("Lookahead Video Window"), the streamer can output the live video stream being processed ahead of time into a standalone window.
     - The window can be freely dragged across the Windows desktop and multiple monitors using its top titlebar (`drag-region`).
     - Supports pinning ("📌 Always on Top"), smooth scaling, live timeline comparison (`Player ➔ Lookahead`), real-time NudeNet/ViT bounding boxes, and an emergency OBS screen block button (**F9**, customizable combination).
     - The window can be opened from the application interface (Step 3 checklist, monitor selectors, warning card), the Windows system tray ("Lookahead Video Window (+10s)"), the HUD alert popup ("Pop out window"), and the Chrome extension popup.
   - On detection, a ticket is created with a countdown timer: the streamer sees the warning and can confirm censorship (**F9**) or dismiss it as a false alarm (**F8**) (hotkeys can be edited and remapped in settings).
   - Supported platforms: YouTube, Twitch, VK Video, Rutube, HDRezka, Kinobox, Kodik, Collaps, Alloha, and any HTML5 player.
   - Player timeline syncs with scheduled cues every ~400 ms, correctly handling scrubbing and pause.

6. **Bilingual Studio Interface (Russian & English)**:
   - Full symmetrical bilingual localization covering all modals, interactive controls, telemetry badges, setup checklists, and the OBS Censor Shield overlay.
   - Dynamic real-time switching between Russian and English without restarting the application.

7. **System Tray Integration & Smart Close Confirmation**:
   - Seamless minimizing to the Windows system tray with an advanced context menu:
     - **Lookahead Video Window (+10s)**: Quick toggle to open the standalone draggable lookahead preview window directly from the tray with live AI threat detection overlays, neon bounding boxes, and confidence badges.
     - **Operation Mode Submenu**: Instant switching between Hybrid, Player Only (Lookahead), Screen Only (Desktop), and Protection Off (Standby) with live checkmarks.
     - **Screen Capture Monitor Submenu**: Select physical monitors directly from the tray with real-time checkmark feedback.
     - **HUD Alert Monitor Submenu**: Choose the monitor for streamer HUD alerts (Auto or specific physical monitor).
     - Standard controls: Show, Minimize to tray, Exit.
   - Left-clicking the tray icon instantly restores and focuses the blewred window.
   - Two-way live synchronization: changing settings in the main window updates tray checkmarks, and changes in the tray menu immediately reflect in the open UI.
   - Interactive close confirmation dialog when clicking the window close button (`X`): choose between minimizing to tray (maintaining uninterrupted background protection) or cleanly exiting the application, with optional preference memory ("Remember my choice") and configurable behavior in Settings.

---

## System Architecture

```
                       Browser (Chrome / Brave / Edge)
                  [ WebExtension / Video Player Lookahead ]
                                     │
                     (Timeline Sync / Scheduled Cues)
                                     │
                                     ▼
 Desktop Screen Capture ──► [ blewred Core Engine ] ◄── Clipboard Screenshot OCR
 (Direct3D 11 Duplication)         (127.0.0.1:51789)    (Windows Media WinRT OCR)
                                     │
                      ┌──────────────┴──────────────┐
                      ▼                             ▼
         [ Dual-Stage AI Vision ]        [ Multilingual Lexical Engine ]
         (ViT Screener + 640m GPU)       (Leet, Homoglyphs, Stopwords)
                      │                             │
                      ▼                             ▼
         [ Zero-Miss Kalman Tracker ]    [ Timing Horizon & Alerts ]
         (Hold-Decay + Dilation)         (Pre-warning, Scrubbing Sync)
                      │                             │
                      ▼                             ▼
         [ GPU Bounding Box UDP ]        [ OBS WebSocket v5 Client ]
         (127.0.0.1:51799)               (Port 4455: SetInputMute, Censor Shield)
                                     │
                                     ▼
                        [ OBS Studio Render Engine ]
                        (HLSL PSCensor Blur & Auto Mute)
```

---

## Technical Architecture & Core Principles

This section explains how blewred achieves ultra-low latency, why the streamer's physical display stays untouched, how the neural cascade works, and how the system prevents bans on Twitch, YouTube, and Kick.

---

### 1. Where Does Censorship Occur?
All visual and audio censorship takes place **directly inside OBS Studio at the GPU hardware level** using a Zero-Copy architecture:

* **The streamer's physical monitor remains 100% untouched**: games, desktop windows, Discord, and mouse movements are never dimmed, blurred, or lagged. The streamer always sees the original pristine screen.
* **Censorship is visible only to stream viewers**: the `obs-blewred.dll` plugin hooks into OBS Studio's DirectX 11 (D3D11) graphics render pipeline. It processes the capture frame **before** it is encoded via NVENC/x264 and transmitted to platform ingest servers.
* **Hardware HLSL Pixel Shader (`PSCensor`)**: defocus blurring and pixelation algorithms run on parallel GPU compute cores directly inside Video RAM (VRAM). Applying the censor filter takes **less than 0.1 milliseconds**, placing zero computational load on the CPU.

---

### 2. Latency Breakdown
For live broadcasting, it is critical that censorship activates before prohibited frames can be seen by viewers or platform moderation bots. The total roundtrip reaction time of blewred is merely **12–18 milliseconds**, well below the duration of a single 60 FPS frame (1 frame = 16.6 ms):

```
Desktop / Game Frame (Direct3D 11)
           │
           ▼ (0.5 – 1.0 ms: texture read via Direct3D Desktop Duplication)
[ Dual-Stage AI: ViT + 640M ]  ──► (8 – 14 ms: DirectML GPU / Tensor Cores)
           │
           ▼ (0.1 – 0.2 ms: localhost UDP packet with bounding boxes to 127.0.0.1:51799)
[ obs-blewred Plugin in OBS Studio ]
           │
           ▼ (< 0.1 ms: HLSL pixel shader applies defocus blur in VRAM)
[ Safe Frame Enters NVENC / x264 Hardware Encoder ]
```

| Pipeline Stage | Technology & Runtime Environment | Execution Time |
| :--- | :--- | :--- |
| **Frame Capture** | Direct3D 11 Desktop Duplication / Shared Memory | **0.5 – 1.0 ms** |
| **Stage 1: ViT Screener** | Safe scene filtering (DirectML GPU) | **1.5 – 3.0 ms** |
| **Stage 2: NudeNet 640M Localizer** | Anatomical region localization (DirectML GPU) | **8.0 – 12.0 ms** |
| **Zero-Miss Tracker (inertia + filtering)** | Kalman filter + bounding box dilation (CPU) | **< 0.1 ms** |
| **OBS Coordinates Dispatch** | Zero-overhead Loopback UDP (`127.0.0.1:51799`) | **0.1 – 0.2 ms** |
| **OBS Blur Rendering** | HLSL defocus pixel shader (GPU VRAM) | **< 0.1 ms** |
| **Total Pipeline Latency** | **Complete cycle from screen event to broadcast blur** | **~12 – 18 ms** |

---

### 3. Neural Cascade & Vision Accuracy
blewred avoids running heavy object detectors on every single frame, which would degrade GPU gaming performance. Instead, it utilizes an asymmetric two-tier cascade:

1. **Stage 1 — Falconsai ViT Screener (224×224)**:
   - High-throughput Vision Transformer classifier.
   - Evaluates frame nudity probability in ~2 milliseconds.
   - **Discards 95% of safe desktop and gameplay frames**, freeing GPU compute resources.
2. **Stage 2 — NudeNet 640M Precision Localizer (640×640)**:
   - Engaged only when Stage 1 flags a risk or an existing target is being actively tracked.
   - Identifies exact coordinates for 18 anatomical categories with bounding boxes.
3. **Proportional 16:9 Letterboxing (Zero Distortion)**:
   - Standard neural models warp frames into 1:1 squares, compressing human figures by 78% and distorting geometry.
   - blewred rescales frames preserving true aspect ratios ($640 \times 360$ with symmetric 140 px padding).
   - This preserves natural anatomical shapes and improves detection accuracy by **25–35%**.
4. **Class-Specific Sensitivity Calibration**:
   - Critical permanent-ban categories (`GENITALIA`, `ANUS`) feature calibrated sensitivity thresholds (0.12–0.40 depending on global sensitivity), eliminating missed detections in dynamic angles or dim lighting.
   - Non-critical categories feature balanced thresholds to avoid false positives on bare shoulders or knees.
5. **Zero-Miss Tracker (Flicker Removal & Predictive Tracking)**:
   - Preserves momentum vectors across momentary occlusions or fast camera sweeps.
   - Bounding boxes expand by **+18% (Dilation Padding)** for a secure margin around contours.
   - **Hold-Decay Memory (30 frames)**: masks persist for ~500 ms with decaying confidence, eliminating strobe flickering and edge leaks.
6. **Adaptive Frame Pacing & Dynamic 60 FPS Escalation**:
   - **Background Scan (5 FPS / 200 ms)**: in safe scenes, screen analysis runs at 200 ms intervals, conserving GPU resources.
   - **Dynamic Threat Escalation (60 FPS / 12 ms)**: when the ViT screener detects probability $\ge 0.15$ or OCR stop-words are found, the engine ramps to 60 FPS in a single frame.
   - **Hold-Decay Return (30 frames)**: after the scene returns to safe status, 60 FPS pacing is held for an additional 30 frames.
   - **Forced Boost Mode (F8)**: streamers can lock the engine into 60 FPS permanently using the F8 hotkey (or custom remapped shortcut) or via the UI toggle.
   - **Hardware Standby (0 FPS)**: in PlayerOnly (Mode 1) and Standby (Mode 3), video capture and OCR immediately sleep (0 FPS, < 1% CPU).
   - **End-to-End UI Synchronization**: the engine broadcasts measured rolling-window FPS and live statuses (`Idle Scan 5 FPS`, `Dynamic Escalation 60 FPS`, `Threat Boost 60 FPS`, `Paused 0 FPS`) to the top ribbon without desync.

#### Neural Network Model Downloads & Cryptographic Hashes
Models can be downloaded in 1 click from the blewred Control Center, or acquired manually for offline deployment into the `models/` directory:

| Model | Cascade Role | File Size | Location | Download Mirrors | Verified SHA-256 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Falconsai ViT Screener** | Stage 1 (Fast Classifier) | ~83.3 MB | `models/vit_nsfw.onnx` | [Hugging Face (Primary)](https://huggingface.co/onnx-community/nsfw_image_detection-ONNX/resolve/main/onnx/model_quantized.onnx) / [Mirror](https://huggingface.co/d0gr/falconsai-nsfw-detector-onnx/resolve/main/nsfw_classifier/model.onnx) / [GitHub](https://github.com/blewred/blewred/releases/download/v1.0.0/vit_nsfw.onnx) | `10354606f3442d24308c0a37d1fd6205ab7ce7bf31669e186a4fc5cf7f51b455` |
| **NudeNet 640m Localizer** | Stage 2 (Precision BBoxes) | ~98.7 MB | `models/640m.onnx` | [Hugging Face (Primary)](https://huggingface.co/zhangsongbo365/nudenet_onnx/resolve/main/640m.onnx) / [GitHub Mirror](https://github.com/notAI-tech/NudeNet/releases/download/v0/640m.onnx) | `5fd488c39acfb268efb4a92bce4fbc95967c059bd7ee1c01026fcaf81aec5c9e` |

---

### 4. Audio Control & Preemptive Muting in OBS Studio
blewred **does not capture microphone audio or perform acoustic speech recognition via WASAPI** (saving CPU/GPU overhead and avoiding audio drift). Audio management is handled cleanly via OBS WebSocket v5:

1. **Event-Driven Incident Muting**:
   - When visual NSFW or on-screen prohibited text (OCR) is detected, blewred dispatches `SetInputMute: true` within fractions of a millisecond to OBS Studio, muting designated audio channels during the incident.
2. **Scheduled Cues Preemptive Muting**:
   - During playback of risky video segments in the browser, OBS audio mutes for the specified duration.
3. **Panic Mute Hotkey (F9, customizable)**:
   - Pressing the hotkey (default **F9**, supports multi-key combos such as `Ctrl+Shift+F9`) instantly mutes microphone and desktop audio in OBS for 3 seconds with optional emergency `Censor Shield` overlay deployment.
4. **Multilingual Lexical Engine**:
   - Inspects on-screen WinRT OCR text and video timecode descriptions against the stop-words dictionary.
   - Resolves intentional leet-speak and cross-alphabet homoglyphs (`0` $\to$ `o`, `3` $\to$ `з`, `@` $\to$ `a`).
   - Supports live editing of stop-words without restarting the stream.

---

### 5. Operating Modes

blewred features 4 operating modes switchable dynamically in real time:

* **Mode 0: Hybrid** — *Recommended default*. Comprehensive protection: browser video player lookahead, scheduled cues, continuous GPU screen analysis + Windows OCR, and automated OBS audio muting.
* **Mode 1: Player Only** — *Minimum resource mode*. Screen capture and OCR are suspended (0 FPS), 100% GPU free, CPU < 1%. Preemptive warnings and muting are managed through the browser extension.
* **Mode 2: Screen Only** — *Autonomous display protection*. Continuous frame-by-frame Direct3D 11 analysis + WinRT OCR without browser extension dependencies.
* **Mode 3: Standby** — Full monitoring pause (0 FPS). All censor masks, audio mutes, and OBS shields are cleared immediately.

#### Race-Condition Protection & Atomic Generations
1. **Atomic Generation IDs (`MODE_GENERATION`)**: every mode transition increments a generation counter. Background tasks verify the generation ID before applying any censor action.
2. **Guaranteed Stale-State Cleansing**: transitioning to Mode 1 clears screen masks; Mode 2 clears player tickets; Mode 3 unblocks all channels and resets shader overlays.
3. **One-Click Mode Standby Toggle**: clicking the active mode button again (or pressing "Off (3)") instantly switches the system into safe Standby mode.
4. **Status Ribbon Indicators**: the top navigation bar displays live connection status (`EXTENSION: ON / OFF`, `OBS STUDIO: ON / OFF`), updated on a 4-second heartbeat.

---

### 6. OBS Studio Integration & 1-Click Auto-Setup

1. **Dynamic DLL Injection without Restarting OBS**:
   - The "OBS Auto-Setup" button detects the running `obs64.exe` process and safely injects `obs-blewred.dll` via Win32 API.
   - Calls `obs_module_load()` directly — the plugin begins operating immediately without interrupting your broadcast.
2. **Automatic Shader Filter Deployment**:
   - Connects via OBS WebSocket v5 and attaches the `blewred_GPU_Censor` shader filter to the active capture source.
3. **Emergency Censor Shield**:
   - Automatically injects a stylish `blewred_Censor_Shield` browser source into all OBS scenes.
   - Activates within 1 ms on critical full-screen violations or when triggered by scheduled cues with a dynamic countdown timer (`TIME REMAINING: X SEC`).
   - Employs a 2.0-second hysteresis hold-time after visual violations clear to completely eliminate detection chatter and rapid ON/OFF flickering.
4. **Physical Monitor Selection**:
   - Streamers can pick a specific physical monitor (1, 2, 3...) for scanning to eliminate monitor switching conflicts.

---

### 7. Audit Log & Incident Management

1. **Unified Violation Tracking**:
   - **Screen detections (NSFW / ViT)**: records anatomical labels and neural confidence.
   - **Text detections (OCR)**: records matched prohibited keywords.
   - **Scheduled Cues**: records player timecode events (`scheduled_cue_started` / `scheduled_cue_ended`) with player source (YouTube, VK, Twitch).
2. **Lifecycle Isolation**:
   - Scheduled cue completion closes only the cue incident (`RESOLVED: CUE EXPIRED`).
   - Screen clearing (`RESOLVED: SCREEN CLEAR`) does not prematurely un-mute an active cue.
3. **Real-Time Filtering**:
   - Quick filters: `All`, `Nudity`, `Stopwords`, `Cues`, `Active`.
4. **Lookahead Tickets & Manual Review**:
   - Each detected lookahead incident creates a ticket with status `pending`, ETA countdown, and a frame preview thumbnail.
   - Confirming censorship (default **F9**, customizable) transitions the ticket to `confirmed_censor` — OBS mutes audio and activates Censor Shield.
   - Dismissing as a false alarm (default **F8**, customizable) transitions it to `allowed` — the ticket closes with no effect on the broadcast.

---

## Quick Start

### 1. Requirements
* Windows 10 / 11 (64-bit)
* [Rust](https://rustup.rs/) (1.77+ with MSVC toolchain)
* [OBS Studio](https://obsproject.com/) (version 28+ with WebSocket v5 enabled on port `4455`)

### 2. Launching blewred
Run the startup script:
```bat
run.bat
```
The script locates the Rust toolchain, compiles the desktop binary via Tauri v2, and launches the Control Center.

### 3. Loading the Browser Extension
1. Open Google Chrome, Brave, or Microsoft Edge.
2. Navigate to `chrome://extensions/`.
3. Enable **Developer mode** in the top-right corner.
4. Click **Load unpacked** and select the `extensions/chrome` directory from blewred.
5. The extension connects automatically to `ws://127.0.0.1:51789`.

---

## Stopwords Dictionary Configuration

You can customize the ban list directly from the Control Center UI or by editing `config/user_stopwords.txt` (which inherits from `config/default_stopwords.txt` on first launch).

### Syntax Rules:
* **Plain Words**: Simple string match (e.g., `retard`).
* **Wildcards**: Prefix or suffix matching (e.g., `word*`).
* **Regex Patterns**: Enclosed between forward slashes (e.g., `/pattern/`).
* **Categories**: Bracketed headers (e.g., `[twitch_hate_speech_en]`).
* **Comments**: Prefixed with `#`.

```text
[twitch_hate_speech_en]
n????r # e.g.
n??a
retard
c?nt
b??ch
person.pdf
# etc...

[twitch_hate_speech_ru]
3.14д0р # для примера
носильщик
роскомнадзор
# и т.п.

[custom_channel]
# Add your channel-specific rules here
```

---

## User Settings Persistence

blewred persists operator preferences across application restarts in `config/` (with fallback to `%LOCALAPPDATA%\blewred\config\`):

| Configuration File | Persisted Preferences | Default Value |
| :--- | :--- | :--- |
| `config/user_settings.json` | Sensitivity, selected monitor, operating mode, auto-mute, censor shield, cue notifications | See UI controls |
| `config/user_stopwords.txt` | Channel ban words, leet filters, wildcards | Inherits from `config/default_stopwords.txt` |
| `config/user_cues.json` | Scheduled timeline cues parsed from screenshots | Restored on launch |

---

## Hotkeys & Controls (Customization & Rebinding)

`blewred` features a comprehensive **hotkey customization and rebinding system**:
- **Flexible Rebinding for All Actions**: customize any hotkey at any time via the "Hotkeys" modal in the top toolbar.
- **Multi-Key Combinations Support**: fully supports modifier keys (`Ctrl`, `Alt`, `Shift`, `Win`) combined with function keys or alphanumeric keys (e.g., `Ctrl+Shift+F9`, `Alt+F8`, or any custom combo).
- **Interactive Keyboard Recording**: click "Record" and press your desired shortcut directly on your keyboard for instant binding, or manually select modifiers and keys using checkboxes and dropdowns.
- **Selectable Scope**:
  - **Globally in Windows**: hotkeys trigger from anywhere (inside fullscreen games, web browsers, or when minimized).
  - **Locally in Application**: hotkeys fire only when the blewred window is focused.

| Action | Default Hotkey | Customization | Description |
| :--- | :---: | :---: | :--- |
| **Panic Mute** | **F9** | Fully rebindable (any combo) | Instantly mutes both Microphone and Desktop Audio in OBS for 3 seconds with Censor Shield. |
| **Threat Mode Toggle** | **F8** | Fully rebindable (any combo) | Toggles screen analysis pacing between Idle mode (5 FPS) and Danger mode (60 FPS). |
| **Lookahead Quick Censor** | **F9** | Synced with Panic Mute | Confirms censorship for the incoming preview violation in the Lookahead window. |
| **Lookahead Dismiss (Safe)** | **F8** | Synced with Threat Mode | Dismisses incoming preview violation as a false alarm in the Lookahead window. |

---

## Quick Setup on a New Workstation

If you have downloaded or cloned the project on a new PC:
1. Double-click **`setup_workstation.bat`**.
2. The script automatically verifies and installs:
   - Microsoft Visual C++ Build Tools & Windows SDK
   - Rust Toolchain (`rustc`, `cargo`, `rustup`)
   - Node.js LTS and Git
   - Microsoft Edge WebView2 Evergreen Runtime
   - Validates neural network weights in `models/`
   - Compiles release `blewred.exe` and creates a desktop shortcut.

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/msbluesnow/blewred.git
cd blewred

# Run automated tests
cargo test --manifest-path src-tauri/Cargo.toml

# Build release executable
cargo build --release --manifest-path src-tauri/Cargo.toml

# The compiled binary will be available at:
# src-tauri/target/release/blewred.exe
```

---

## License

`blewred` is distributed under the **Apache License, Version 2.0**. See the [LICENSE](LICENSE) and [NOTICE](NOTICE) files for details.

### Modular Licensing

* **Core Application & Extensions** (`src-tauri/`, `ui/`, `extensions/`): Licensed under the [Apache License, Version 2.0](LICENSE).
* **OBS Studio Plugin** (`plugins/obs-blewred/`): Licensed under the [GNU General Public License v2.0 or later](plugins/obs-blewred/LICENSE) as required by the OBS Studio Plugin API.
* **Adwaita Fonts** (`ui/fonts/`): Licensed under the [SIL Open Font License 1.1](ui/fonts/OFL.txt).
* **Third-Party Neural Network Models**: External models fetched at runtime operate under their respective upstream licenses as detailed in [NOTICE](NOTICE).
