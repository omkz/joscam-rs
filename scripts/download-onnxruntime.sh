#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
    echo "Automatic runtime setup supports Linux x86_64. Supply a compatible ONNX Runtime 1.19 library through ORT_DYLIB_PATH on other systems." >&2
    exit 1
fi
runtime_dir="$project_dir/models/onnxruntime"
runtime_lib="$runtime_dir/lib/libonnxruntime.so.1.19.2"
lib_sha=9c8139e1856568f068f1cad78fc9798084941db50be7c201a1dbdcfd18105433
archive_sha=eb00c64e0041f719913c4080e0fed7d9963dc3aa9b54664df6036d8308dbcd33
if [[ -f "$runtime_lib" ]] && printf '%s  %s\n' "$lib_sha" "$runtime_lib" | sha256sum --check --status; then
    echo "ONNX Runtime already verified: $runtime_lib"
    exit 0
fi
if [[ -e "$runtime_dir" ]]; then
    echo "Existing runtime directory is incomplete or differs from the pinned release. Move it aside before retrying: $runtime_dir" >&2
    exit 1
fi
mkdir -p "$project_dir/models"
runtime_tmp="$(mktemp -d "$project_dir/models/.ort.XXXXXX")"
trap 'rm -rf -- "$runtime_tmp"' EXIT
curl --fail --location --retry 2 --connect-timeout 15 --max-time 180 \
  https://github.com/microsoft/onnxruntime/releases/download/v1.19.2/onnxruntime-linux-x64-1.19.2.tgz \
  --output "$runtime_tmp/runtime.tgz"
printf '%s  %s\n' "$archive_sha" "$runtime_tmp/runtime.tgz" | sha256sum --check --status
tar --no-same-owner -xzf "$runtime_tmp/runtime.tgz" -C "$runtime_tmp"
printf '%s  %s\n' "$lib_sha" "$runtime_tmp/onnxruntime-linux-x64-1.19.2/lib/libonnxruntime.so.1.19.2" | sha256sum --check --status
mv -- "$runtime_tmp/onnxruntime-linux-x64-1.19.2" "$runtime_dir"
echo "ONNX Runtime verified: $runtime_lib"
