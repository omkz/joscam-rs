# Face detection model

Run `bash scripts/download-face-model.sh` once from any directory. The script
fetches the model from atomashpolskiy/rustface revision
`fa3d5b5d576c91d1c8a099af381e0f0906a0d3a3`, verifies SHA-256
`c4619d066ed35e84d9a8e842860b0dff567aba0cbb139881075538761db3ff5d`, and
places it here. No camera data is uploaded. Runtime inference is local.

Source: https://github.com/atomashpolskiy/rustface
Model: `model/seeta_fd_frontal_v1.0.bin` (~1.2 MB), SeetaFace detector.
See `third-party/rustface-LICENSE.txt` for the upstream BSD-2-Clause notice.
Include that notice with redistributed model/library binaries.

## Dense face landmarks

`bash scripts/download-landmark-model.sh` downloads the 468-point 192x192 RGB
Face Mesh ONNX export from:
https://github.com/yakhyo/mediapipe-face-mesh-onnx/releases/tag/weights

SHA-256: `3ca77cf59c18e4da0eccb46695bf604683fa564253e3385892981a5c274fb10f`
Source: https://github.com/yakhyo/mediapipe-face-mesh-onnx
The export derives from Google's MediaPipe Face Mesh and is distributed under
Apache-2.0. See `third-party/face-mesh-APACHE-2.0.txt` and
`third-party/FACE_MESH_NOTICE.md`. Preserve notices when redistributing.
The script fails if upstream replaces the weight with different bytes.

## Local CPU inference runtime

`bash scripts/setup-beauty.sh` performs all model/runtime downloads. For the
runtime alone use `bash scripts/download-onnxruntime.sh` (Linux x86_64).
Official release: https://github.com/microsoft/onnxruntime/releases/tag/v1.19.2
Archive: `onnxruntime-linux-x64-1.19.2.tgz`
Archive SHA-256: `eb00c64e0041f719913c4080e0fed7d9963dc3aa9b54664df6036d8308dbcd33`
Library SHA-256: `9c8139e1856568f068f1cad78fc9798084941db50be7c201a1dbdcfd18105433`
The extracted runtime directory retains Microsoft's MIT license. No binaries
are committed. `ORT_DYLIB_PATH` can override the default library location.
