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
