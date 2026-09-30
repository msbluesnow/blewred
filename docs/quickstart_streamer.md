# blewred Streamer Quick Start Guide

> **Simple step-by-step setup guide without complicated technical jargon.**  
> Follow 4 simple steps to prepare blewred for secure live streaming on Twitch, YouTube, and Kick.

---

## Pre-Broadcast Readiness Checklist

```
[ Step 1: Nvidia GPU Acceleration ] ──► [ Step 2: AI Vision Models ] ──► [ Step 3: Browser Extension ] ──► [ Step 4: OBS Studio ]
           DirectML & HAGS                        ViT + NudeNet 640m                  Timeline Sync                     Plugin & Shield
```

---

## Step 1: Nvidia GPU Hardware Acceleration (DirectML)

blewred runs neural networks directly on your graphics card via **Microsoft DirectML**. This inference takes merely **8–14 milliseconds** and places virtually zero burden on your CPU, ensuring game performance and FPS remain perfectly smooth.

> [!TIP]
> **Do I need to install the NVIDIA CUDA Toolkit or cuDNN?**  
> **NO, you do not!** Unlike Python environments, blewred uses native **Microsoft DirectML** on top of DirectX 12 Compute. To run AI models on an NVIDIA GPU, **a standard graphics driver is all that is required** (GeForce Game Ready or Studio Driver version 535+). No large developer packages (CUDA/cuDNN 3–5 GB), PATH environment variables, or compilers are needed.

### 1.1. Check and Update NVIDIA Driver
Make sure you have an up-to-date NVIDIA driver installed (Game Ready or Studio Driver):
1. Open the **NVIDIA App** or **GeForce Experience** and navigate to the **Drivers** tab.
2. If an update is available, click **Download** and select **Express Installation** (driver version 535.xx or newer recommended).
3. Alternatively, download the driver from the official site [nvidia.com/drivers](https://www.nvidia.com/download/index.aspx).

### 1.2. Select High-Performance GPU in Windows 10/11
Crucial for **gaming laptops** and PCs with integrated graphics (Intel UHD / Iris Xe / AMD Radeon):
1. Open **Windows Settings** (`Win + I`) -> **System** -> **Display** -> **Graphics** (or click **Windows Graphics Settings** inside blewred).
2. Locate `blewred.exe` in the application list. If not present, click **Browse** and select `blewred.exe` from the program directory.
3. Click **Options** next to `blewred.exe`.
4. Select **High performance** (with your NVIDIA GeForce GPU) and click **Save**.

### 1.3. Enable Hardware-Accelerated GPU Scheduling (HAGS)
HAGS cuts screen capture latency by 2–3x:
1. In the *Graphics settings* window, click **Change default graphics settings**.
2. Turn on the toggle for **Hardware-accelerated GPU scheduling (HAGS)**.
3. Restart your computer for changes to take effect.

### 1.4. NVIDIA Control Panel Optimization
1. Right-click your desktop and open **NVIDIA Control Panel**.
2. Go to **Manage 3D Settings** -> **Program Settings** tab.
3. Click **Add** and select `blewred.exe`.
4. Locate **Power management mode** and set it to **Prefer maximum performance**.
5. Click **Apply** in the bottom-right corner.

---

## Step 2: AI Neural Network Models

blewred employs a high-performance two-stage cascade:
* **Falconsai ViT Screener**: Discards 95% of safe gaming/desktop frames in ~2 ms.
* **NudeNet 640m Localizer**: Identifies precise bounding coordinates for prohibited anatomical regions to apply GPU blurring.

### 1-Click Verification & Download:
1. In the blewred Control Center, click **Check / Download Models** in the Neural Network Models card or top menu.
2. The application verifies the presence of `vit_nsfw.onnx` and `640m.onnx` in `models/` along with their SHA-256 checksums.
3. If the status indicates `Missing` or `Corrupted`, click **Start Download**.
4. Files download automatically in the background with live speed and ETA telemetry.

### Direct Manual Download Links (Offline / Mirrors):
If you are setting up an offline system or prefer downloading model weights manually via your browser or a download manager, download both `.onnx` files and save them to the `models/` directory in the application root:

| Model | Stage | File Size | Destination in `models/` | Direct Download Links | SHA-256 Checksum |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Falconsai ViT Classifier** | Stage 1 (Fast Screener) | ~83.3 MB | `models/vit_nsfw.onnx` | • [Hugging Face (Primary)](https://huggingface.co/onnx-community/nsfw_image_detection-ONNX/resolve/main/onnx/model_quantized.onnx)<br>• [Hugging Face Mirror](https://huggingface.co/d0gr/falconsai-nsfw-detector-onnx/resolve/main/nsfw_classifier/model.onnx)<br>• [GitHub Mirror](https://github.com/blewred/blewred/releases/download/v1.0.0/vit_nsfw.onnx) | `10354606f3442d24308c0a37d1fd6205ab7ce7bf31669e186a4fc5cf7f51b455` |
| **NudeNet 640m Localizer** | Stage 2 (Precision Detector) | ~98.7 MB | `models/640m.onnx` | • [Hugging Face (Primary)](https://huggingface.co/zhangsongbo365/nudenet_onnx/resolve/main/640m.onnx)<br>• [GitHub Mirror (notAI-tech)](https://github.com/notAI-tech/NudeNet/releases/download/v0/640m.onnx) | `5fd488c39acfb268efb4a92bce4fbc95967c059bd7ee1c01026fcaf81aec5c9e` |

> [!NOTE]
> If the primary Hugging Face download saves as `model_quantized.onnx` or `model.onnx`, simply rename the file to **`vit_nsfw.onnx`** before placing it into the `models/` folder. The application verifies SHA-256 checksums automatically.

---

## Step 3: Browser Extension (HTML5 Video Player Timeline Sync)

The browser extension bridges HTML5 video players (YouTube, VK Video, Twitch, etc.) to the blewred core engine at `ws://127.0.0.1:51789`. It continuously syncs playback position (`currentTime`), scrubbing, and pause events with Scheduled Cues for advance warnings and automatic censorship.

### Installation Guide (Google Chrome, Brave, Edge, Opera):
1. Open your browser and navigate to the extensions page:
   * **Chrome**: `chrome://extensions/`
   * **Brave**: `brave://extensions/`
   * **Edge**: `edge://extensions/`
2. Enable **Developer mode** toggle in the top-right corner.
3. Click **Load unpacked**.
4. Select the **`extensions/chrome`** directory from the blewred folder (or click **Open Extension Folder** in blewred's checklist).
5. Click **Select Folder**.
6. Pin the blewred icon in your browser toolbar. The **Extension** badge in blewred's header will light up green (**ON**).

---

## Step 4: OBS Studio Integration & Safety Checks

blewred applies blurring and overlays **directly inside OBS Studio's DirectX 11 render pipeline** before the frame reaches encoding. Your own display remains completely unblurred and distraction-free.

### 4.1. Enable WebSocket in OBS Studio
1. Open **OBS Studio** (version 28.0 or newer).
2. Go to **Tools** -> **WebSocket Server Settings**.
3. Check **Enable WebSocket server**.
4. Verify the server port is set to **`4455`**.
5. If password protection is enabled, enter the password in blewred's OBS settings (default has password disabled).

### 4.2. 1-Click Auto-Setup
1. In blewred, click **OBS Auto-Setup**.
2. The application automatically:
   - Injects the high-speed `obs-blewred.dll` plugin.
   - Configures the hardware shader filter on the active capture source.
   - Deploys the stylish **Censor Shield** (`blewred_Censor_Shield`) emergency overlay across all broadcast scenes with real-time bilingual localization (RU/EN).

### 4.3. Monitor Selection for Analysis
The **Stream Monitor** dropdown lists all connected physical displays (e.g., "Monitor 1 (Primary)", "Monitor 2", "Monitor 3"):
* Select the display displaying your game or streaming content.
* Neural vision and OCR scanning operate **strictly on the selected physical monitor**, eliminating desync and false display switching.

### 4.4. Streamer Hotkeys
Verify emergency shortcuts:
* **F9 (Panic Mute / Emergency Censor)**: Instantly mutes microphone and desktop audio in OBS for 3 seconds and deploys the Censor Shield overlay.
* **F8 (Threat Boost Mode)**: Escalates screen analysis from 5 FPS to 60 FPS (useful when navigating risky sites or viewing chat media).

> 💡 **Hotkey Customization**: In `blewred`, all hotkeys can be fully customized and remapped via the **"Hotkeys"** button in the top toolbar. Multi-key combinations with modifiers (`Ctrl`, `Alt`, `Shift`, `Win`), interactive keyboard recording, and scope selection (global across Windows or local to the app) are fully supported.

---

## Frequently Asked Questions (FAQ) & Troubleshooting

| Issue | Cause | Simple Solution |
| :--- | :--- | :--- |
| Censorship activates on the wrong display | A different monitor is selected in blewred | Select the correct physical monitor in the "Stream Monitor" dropdown. blewred scans strictly that monitor. |
| "Extension" indicator shows gray (OFF) | Extension is not running or browser is closed | Open your browser with the extension loaded. Ensure a video page is open. |
| "OBS Studio" indicator shows OFF | WebSocket server is disabled in OBS | In OBS Studio, open *Tools -> WebSocket Server Settings* and enable the server on port 4455. |
| Vision inference runs on CPU instead of GPU | Windows assigned integrated graphics | Follow **Step 1.2** in this guide to assign High Performance NVIDIA GPU to `blewred.exe`. |
| Model files are missing or corrupted | Files were not downloaded on first launch | Click **Check / Download Models** in the Control Center to start background download. |
| How to dismiss the setup checklist? | Setup is already complete | Click the close button (`×`) in the top right of the panel or check "Do not show on startup". You can reopen it anytime via "Setup Guide" in the header. |
