# Face Mesh attribution

Model family: Google MediaPipe Face Mesh (468 facial landmarks).
Original project: https://github.com/google-ai-edge/mediapipe

ONNX export: Yakhyokhuja Valikhujaev, mediapipe-face-mesh-onnx.
Copyright 2026 Yakhyokhuja Valikhujaev.
Source: https://github.com/yakhyo/mediapipe-face-mesh-onnx
The export reconstructs the MediaPipe network and its released weights.
License: Apache License 2.0; the full upstream license is included in
face-mesh-APACHE-2.0.txt.

Joscam's preprocessing and contour rendering are independent Rust
implementations. Input is one 192x192 RGB crop scaled to [0,1]; output is
468 crop-space landmarks and a face-presence logit. Canonical landmark
indices follow the MediaPipe topology. This project is not affiliated with
or endorsed by Google or the ONNX export author.
