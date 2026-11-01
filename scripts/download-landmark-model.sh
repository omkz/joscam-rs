#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
model_path="$project_dir/models/face_mesh_Nx3x192x192.onnx"
model_sha=3ca77cf59c18e4da0eccb46695bf604683fa564253e3385892981a5c274fb10f
mkdir -p "$project_dir/models"
if [[ -f "$model_path" ]] && printf '%s  %s\n' "$model_sha" "$model_path" | sha256sum --check --status; then
    echo "Landmark model already verified: $model_path"
    exit 0
fi
model_tmp="$(mktemp "$project_dir/models/.face-mesh.XXXXXX")"
trap 'rm -f -- "$model_tmp"' EXIT
curl --fail --location --retry 2 --connect-timeout 15 --max-time 180 \
  https://github.com/yakhyo/mediapipe-face-mesh-onnx/releases/download/weights/face_mesh_Nx3x192x192.onnx \
  --output "$model_tmp"
printf '%s  %s\n' "$model_sha" "$model_tmp" | sha256sum --check --status
mv -- "$model_tmp" "$model_path"
echo "Landmark model verified: $model_path"
