#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
bash "$script_dir/download-face-model.sh"
bash "$script_dir/download-landmark-model.sh"
bash "$script_dir/download-onnxruntime.sh"
