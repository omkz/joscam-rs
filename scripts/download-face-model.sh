#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
model_dir="$project_dir/models"
model_path="$model_dir/seeta_fd_frontal_v1.0.bin"
model_sha=c4619d066ed35e84d9a8e842860b0dff567aba0cbb139881075538761db3ff5d
upstream_commit=fa3d5b5d576c91d1c8a099af381e0f0906a0d3a3
mkdir -p "$model_dir"
if [[ -f "$model_path" ]] && printf '%s  %s\n' "$model_sha" "$model_path" | sha256sum --check --status; then
    echo "Face model already verified: $model_path"
    exit 0
fi
model_tmp="$(mktemp -d)"
trap 'rm -rf -- "$model_tmp"' EXIT
# Fetch one pinned upstream revision; never execute upstream code.
git -C "$model_tmp" init -q
git -C "$model_tmp" fetch -q --depth=1 https://github.com/atomashpolskiy/rustface.git "$upstream_commit"
git -C "$model_tmp" show "$upstream_commit:model/seeta_fd_frontal_v1.0.bin" > "$model_tmp/model.bin"
printf '%s  %s\n' "$model_sha" "$model_tmp/model.bin" | sha256sum --check --status
mv -- "$model_tmp/model.bin" "$model_path"
echo "Face model verified: $model_path"
