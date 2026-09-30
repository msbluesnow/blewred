# blewred — Preemptive Live Stream Protection Suite

> **Preemptive, Zero-Overhead Live Stream Compliance & AI Vision Shield**  
> *“Use protection B4 it happens...”*

Welcome to the official technical documentation for **blewred**, an open-source real-time broadcast compliance engine designed for content creators streaming on **Twitch**, **YouTube**, and **Kick**.

> 💡 **New to blewred?** Check out the **[Streamer Quick Start Guide](/quickstart_streamer)** (Russian: **[Руководство для стримеров](/ru/quickstart_streamer)**) for 4 simple setup steps (Nvidia GPU, Extension, Models, OBS Studio).

> [!IMPORTANT]
> **Project Status: Alpha Stage (v0.5.0-alpha)**
> The `blewred` suite is currently in active alpha development. Occasional edge cases or temporary limitations may occur. New features, architectural refinements, and performance optimizations are actively being planned and developed. We warmly welcome bug reports and feedback on [GitHub Issues](https://github.com/msbluesnow/blewred/issues)!

blewred prevents platform bans by intercepting prohibited visual content (NSFW/nudity, prohibited symbols), on-screen prohibited text (OCR), and scheduled video player cues (HTML5 player timeline sync) **before** they reach viewers or ingestion servers, triggering real-time GPU defocusing and audio muting in OBS Studio.

---

## 1. System Architecture & Core Concept

blewred operates on an asynchronous, decoupled pipeline designed for minimal CPU and GPU overhead (< 70 MB RAM, < 1% CPU in PlayerOnly mode, < 15 ms GPU inference).

### High-Level Block Diagram

```
+-----------------------------------------------------------------------------+
|                               Browser Context                               |
|   [ WebExtension: HTML5 Video Player Timecode Sync (YouTube/VK/Twitch) ]    |
|                                     |                                       |
|                  Ahead-of-Time Cues / Sync (Adaptive Lead)              |
+-------------------------------------|---------------------------------------+
                                      v
+-----------------------------------------------------------------------------+
|                             blewred Core Engine                             |
|                        WebSocket Daemon (127.0.0.1:51789)                   |
|                                                                             |
|   +--------------------------+          +-------------------------------+   |
|   |   D3D11 Screen Capture   |          |   Windows Media OCR Engine    |   |
|   |   (Desktop Duplication)  |          |   (Offline WinRT API)         |   |
|   +------------+-------------+          +---------------+---------------+   |
|                v                                        v                   |
|   +--------------------------+          +-------------------------------+   |
|   | Dual-Stage AI Cascade    |          | Multilingual Lexical Engine   |   |
|   | Stage 1: ViT (224x224)   |          | Homoglyph & Leet Normalizer   |   |
|   | Stage 2: 640m Localizer  |          | Regex & Ban Words Dictionary  |   |
|   +------------+-------------+          +---------------+---------------+   |
|                v                                        v                   |
|   +--------------------------+          +-------------------------------+   |
|   | Zero-Miss Kalman Tracker |          | Scheduled Cues Manager        |   |
|   | Inertia + Hold-Decay     |          | Lead Horizon & Scrubbing Sync |   |
|   +------------+-------------+          +---------------+---------------+   |
|                |                                        |                   |
+----------------|----------------------------------------|-------------------+
                 | UDP Datagrams (127.0.0.1:51799)        | WebSocket v5 (4455)
                 v                                        v
+-----------------------------------------------------------------------------+
|                                 OBS Studio                                  |
|   +------------------------------------+   +----------------------------+   |
|   | Plugin obs-blewred.dll (Native C)  |   | obs-websocket v5 Service   |   |
|   | D3D11 PSCensor Pixel Shader        |   | SetInputMute (Auto Mute)   |   |
|   | Zero-Copy VRAM Blurring            |   | Censor Shield Splash Screen|   |
|   +------------------------------------+   +----------------------------+   |
+-----------------------------------------------------------------------------+
```

---

## 2. Real-Time Operating Modes

blewred features 4 operating modes switchable dynamically on the fly without restarting OBS Studio or background processes:

| ID | Mode Name | Screen Capture | Video Player (Extension) | CPU Overhead | Purpose & Recommended Usage |
| :---: | :--- | :---: | :---: | :---: | :--- |
| **0** | **Hybrid** *(Default)* | Active (GPU/OCR) | Active (Lookahead) | 1–3% | Complete protection suite. Recommended for standard live streams. |
| **1** | **Player Only** | Paused (0 FPS) | Active (Lookahead) | < 1% | Ideal for video reaction streams with maximum gaming FPS. |
| **2** | **Screen Only** | Active (GPU/OCR) | Disabled | 1–2% | Autonomous screen & game protection without browser extension dependencies. |
| **3** | **Standby** | Paused (0 FPS) | Paused | < 0.1% | Monitoring paused. All censor masks and audio mutes are cleared immediately. |

### Race-Condition Protection & Atomic Generations (MODE_GENERATION)
To prevent stale censor masks or delayed timers from applying after switching modes, each transition is assigned a unique generational ID (`MODE_GENERATION`). Asynchronous background tasks verify the generation ID before dispatching any command to OBS Studio.

---

## 3. Computer Vision & Neural Cascade

### Dual-Stage AI Cascade
Rather than running heavy object detectors on every single frame, blewred utilizes an asymmetric two-tier cascade:

1. **Stage 1 — Falconsai ViT Screener (224×224)**:
   - Evaluates frame nudity probability in ~1.5–3.0 ms via DirectML / ONNX Runtime.
   - Discards over 95% of safe desktop and gameplay frames without loading the localizer.
2. **Stage 2 — NudeNet 640m Localizer (640×640)**:
   - Engaged only when Stage 1 flags a risk or an existing target is being actively tracked.
   - Identifies exact coordinates for 18 anatomical categories with true 16:9 Letterboxing ($640 \times 360$ with symmetric padding).

### Zero-Miss Tracker & Hysteresis
- **Adaptive Dilation (+18%)**: Detected bounding boxes are automatically expanded by 18% to prevent edge leaks during movement.
- **Hold-Decay Memory (30 frames)**: Active masks persist for 30 frames (~500 ms) with decaying confidence, eliminating strobe flickering and edge chatter.
- **Dynamic 60 FPS Escalation**: When danger is flagged, analysis pacing ramps from 5 FPS (background idle) to 60 FPS (12 ms interval) in a single frame.

---

## 4. Text Recognition (OCR) & Player Cues

### Windows Media OCR Engine
blewred uses the built-in Windows WinRT OCR engine (`windows::Media::Ocr::OcrEngine`):
- **Zero Heavy Models**: Built into Windows 10/11, requiring no heavy external weights.
- **100% Offline**: Operates strictly on-device without cloud communication.
- **Bilingual**: Native recognition of Russian and English text with homoglyph/leet-speak defense.

### Scheduled Cues Module (Fuzzy Timecode Parsing)
Streamers can paste screenshots of timecodes directly from the clipboard (`Ctrl+V`):
- **Primary Whitespace Normalization**: Automatically strips OCR kerning spaces around colons (`12 : 30` ➔ `12:30`), dots, and numbers before parsing.
- **Syntax Normalization**: Handles non-standard formats (`12:30-14:15`, seconds > 59, non-standard dashes).
- **Scrubbing Tolerance**: Recalculates lead times dynamically if the streamer fast-forwards or rewinds.
- **Pre-Warning Modal**: Displays countdown alerts with manual override capabilities.

---

## 5. OBS Studio Integration (`obs-blewred`)

### Zero-Copy Hardware GPU Blurring
The native plugin (`obs-blewred.dll`) is injected directly into the 64-bit OBS Studio process:
- **D3D11 Pipeline Hook**: Defocus blurring runs via an HLSL pixel shader (`PSCensor`) directly in VRAM before video encoding.
- **Clean Streamer Display**: The streamer's monitor is untouched; censorship applies exclusively to the outgoing broadcast.
- **UDP Loopback Dispatch**: Coordinates are sent from blewred to OBS via localhost UDP (`127.0.0.1:51799`) in < 0.2 ms.

### Emergency Censor Shield
A stylish browser source (`blewred_Censor_Shield`) is injected into OBS scenes. In major incidents, the system activates full-screen censorship within 1 ms and mutes audio.

### Automatic Audio Control (OBS WebSocket v5)
blewred **does not capture microphone audio through WASAPI or perform speech recognition**, avoiding CPU overhead and audio desync. Control is executed via OBS WebSocket v5:
- **Auto-Discovery**: Identifies microphone and desktop audio sources in OBS automatically.
- **Instant Muting**: Sends `SetInputMute: true` within fractions of a millisecond during incidents.
- **Seamless Recovery**: Audio un-mutes automatically once the violation clears.

---

## 6. WebSocket Protocol Specification

blewred provides a local asynchronous WebSocket bridge on `ws://127.0.0.1:51789`.

### Incoming Messages (Client -> Server)

| `type` | Parameters | Description |
| :--- | :--- | :--- |
| `set_language` | `language: "ru" \| "en"` | Switch and persist language; synchronizes UI and all connected clients including OBS Censor Shield. |
| `set_operation_mode` | `mode: number (0..3)` | Switch operating mode. |
| `toggle_censor_shield` | `enabled: boolean` | Toggle Censor Shield browser source overlay in OBS scenes. |
| `toggle_shield_donate` | `enabled: boolean` | Toggle developer support banner on Censor Shield overlay. |
| `set_ocr_enabled` | `enabled: boolean` | Toggle OCR stop-word detection. |
| `set_nsfw_threshold` | `threshold: number (0.0..1.0)` | Adjust neural sensitivity threshold. |
| `add_scheduled_cues` | `cues: Cue[], append: boolean` | Add or replace scheduled cues. |
| `delete_scheduled_cue`| `id: string` | Remove cue by ID. |
| `clear_scheduled_cues`| *(none)* | Clear all cues. |
| `set_cues_config` | `pre_warning_seconds, auto_censor, append_mode` | Update scheduler preferences. |
| `recognize_screenshot_cues` | `image: string (base64)` | Recognize cues from clipboard screenshot via Windows OCR. |
| `check_models_status` | *(none)* | Check ONNX model checksums and presence. |
| `start_models_download`| *(none)* | Download missing neural network models in background. |

### Outgoing Events (Server -> Client)

| `type` | Parameters | Description |
| :--- | :--- | :--- |
| `init_state` | `language, obs_connected, rules_text, models_status, ...` | Comprehensive initial state sent immediately upon client connection. |
| `language_changed` | `language: "ru" \| "en"` | Broadcasted notification when interface language is updated. |
| `telemetry` | `fps, gpu_info, obs_connected, ocr_enabled, ...` | Live performance metrics and status. |
| `shield_donate_changed` | `enabled: boolean` | Broadcasted notification when developer support banner visibility toggles. |
| `scheduled_cues_updated` | `cues: Cue[]` | Notification of updated cue list. |
| `cues_state_result` | `cues, config, player_sync` | Response to cues state query. |
| `recognized_cues_result` | `cues, raw_text, count` | OCR screenshot parsing result. |
| `lookahead_ticket_created` | `ticket_id, reason, eta_sec, preview_base64` | Upcoming violation warning event. |
| `lookahead_ticket_resolved` | `ticket_id, action` | Incident resolved or dismissed event. |

---

## 7. Building & Development

### Requirements
- **OS**: Windows 10 / 11 (64-bit)
- **Rust Toolchain**: 1.77+ with MSVC (`rustup default stable-x86_64-pc-windows-msvc`)
- **C/C++ Build Tools**: Visual Studio 2022 or Build Tools with Windows SDK
- **Node.js**: v18+ LTS
- **OBS Studio**: Version 28+ (WebSocket v5 on port 4455)

### Build Commands

```bash
# 1. Clone the repository
git clone https://github.com/msbluesnow/blewred.git
cd blewred

# 2. Run automated tests
cargo test --manifest-path src-tauri/Cargo.toml

# 3. Build release executable
cargo build --release --manifest-path src-tauri/Cargo.toml

# 4. Build OBS native plugin (optional)
plugins\obs-blewred\build_plugin.bat
```

The compiled binary will be located at:
`src-tauri/target/release/blewred.exe`

---

## 8. Hotkeys & Controls (Customization & Rebinding)

`blewred` features a comprehensive **hotkey customization and rebinding system**:
- **Panic Mute (Emergency Mute)**: Default **F9** (instantly mutes microphone & desktop audio in OBS for 3 seconds + activates Censor Shield).
- **Threat Boost Mode**: Default **F8** (toggles video analysis between 5 FPS background idle and 60 FPS danger mode).
- **Full Rebinding**: Supports complex combinations with modifier keys (`Ctrl`, `Alt`, `Shift`, `Win` + any key), interactive recording directly from the keyboard, and selectable scope (globally across Windows or locally in the application).

---

## 9. UI Design Standards

The blewred UI strictly conforms to the **Bootstrap V3 Dark Edition** design specification:
- Panels (`.panel`, `.panel-default`, dark background `#1a1f29`, borders `#2d3748`).
- Buttons (`.btn`, `.btn-default`, `.btn-primary`, `.btn-success`, `.btn-warning`, `.btn-danger`, `.btn-sm`, `.btn-xs`).
- Badges and labels (`.label`, `.badge`, `.label-default`, `.label-success`, `.label-warning`, `.label-danger`).
- **Strict Prohibition of HTML `title` Attributes**: Browser hover tooltips are forbidden across all markup. All action hints and hotkeys are displayed directly in the visible text of elements.

---

## 10. License

`blewred` is distributed under the **Apache License, Version 2.0**. See the [LICENSE](https://github.com/msbluesnow/blewred/blob/main/LICENSE) file for complete details.
