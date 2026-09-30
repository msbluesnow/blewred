==============================================================================
blewred - Neural Network Models Directory
==============================================================================

This folder is used by blewred to store local ONNX neural network models
for hardware-accelerated NSFW detection (ViT) and anatomical localization (NudeNet).

Expected files:
  1. vit_nsfw.onnx (Preemptive Scene Screening Model)
  2. 640m.onnx     (NudeNet Anatomical Bounding-Box Localizer)

How to install models:
  - Automatic (Recommended):
      Launch blewred -> Open "Streamer Quick Start" -> Step 2 -> Click "Download Models"
      The application will download models directly with SHA-256 integrity verification.
  - Manual:
      Copy your pre-downloaded "vit_nsfw.onnx" and "640m.onnx" directly into this folder.

blewred will detect the models immediately upon launch.
==============================================================================
