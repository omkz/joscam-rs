# Joscam Rust

Local Linux webcam beauty effects and virtual camera for Chrome. Rust processes
frames; FFmpeg handles capture and V4L2 output. Camera frames stay on your machine.

## Install on Linux Mint

Install Rust through rustup, then use the tested toolchain:

```bash
rustup toolchain install 1.85.1
sudo apt install ffmpeg v4l-utils v4l2loopback-dkms linux-headers-$(uname -r)
git clone https://github.com/omkz/joscam-rs.git
cd joscam-rs
bash scripts/setup-beauty.sh
cargo +1.85.1 build --release --locked
```

Setup downloads checksum-verified face detection, 468-point face mesh, and
ONNX Runtime 1.19.2. Automatic runtime setup supports Linux x86_64. For custom
installations set `ORT_DYLIB_PATH` to an API 19 compatible runtime library.
Models and runtime are not committed. Run commands from the project root.

## Control panel

On the PR branch `feature/particles-overlay`:

```bash
cargo +1.85.1 run --release --locked -- --panel --preset kbeauty
```

The app opens a local browser panel; if opening fails, copy the URL printed in
the terminal. It provides a preview, live sliders for smoothing, skin brightness,
face light, rosy tint, glow, lips and eyes, plus glasses and particle controls.
No browser camera permission is needed for this preview.

Virtual output starts **off**. Prepare the device below, then click **Mulai
virtual camera**. Preview continues while Chrome uses the virtual camera.
**Berhenti** stops only virtual output; preview keeps the physical webcam open.
Use **Ctrl+C in the terminal** to exit and release the camera. Closing the panel
tab does not stop the app. Output failures appear in the panel; fix the device
and click start again without restarting the preview.

The panel binds only to 127.0.0.1 on a random port with a random URL token. It
loads no external assets. Preview is downscaled to at most 320 pixels wide and
updated at up to 10 FPS to limit overhead; virtual output retains the configured
resolution and frame rate. The panel is a local browser UI, not a native window.
It cannot combine with `--preview`, `--compare`, or `--record`.

## Virtual camera for Chrome / OmeTV

Close other camera producers, then prepare the loopback device:

```bash
sudo modprobe v4l2loopback devices=1 video_nr=2 exclusive_caps=1 card_label="Joscam Rust"
v4l2-ctl --list-devices
```

Use the panel's start button, or run directly:

```bash
cargo +1.85.1 run --release --locked -- \
  --input /dev/video0 --output /dev/video2 --preset kbeauty
```

Start output before opening Chrome's camera settings, then select **Joscam Rust**.
The physical webcam light being on is normal. Chrome uses the processed virtual
camera, while microphone selection stays in Chrome. If the camera is missing,
restart Chrome after starting output. Websites may reject virtual cameras;
no site-specific detection bypass is implemented.

Never use the physical webcam as `--output`. Default input is `/dev/video0`, output
`/dev/video2`, resolution 640x480, requested FPS 30. Device numbers vary by machine.

## Standalone preview and comparison

These modes replace virtual output. Use the panel for simultaneous preview/output.

```bash
cargo +1.85.1 run --release --locked -- --preview --preset kbeauty
cargo +1.85.1 run --release --locked -- --preview --preset kbeauty --compare
```

Q, Esc, or closing the FFplay window stops standalone preview. Comparison shows
original on the left, processed on the right. FFplay requires a graphical desktop.

## Effects

```bash
cargo +1.85.1 run --release --locked -- --panel --preset kbeauty \
  --strength 0.85 --skin-brightening 0.65 --lip-tint 0.5 --eye-brightening 0.5 \
  --glasses sunglasses --particles both --particle-count 25
```

| Option | Values / purpose |
| --- | --- |
| `--preset` | `natural`, `youth`, `kbeauty` |
| `--strength` | Smoothing, 0..1 |
| `--light` | Face light, 0..1 |
| `--skin-brightening` | Skin brightness, 0..1 |
| `--rosy`, `--glow` | Rosy tint and glow, 0..1 |
| `--lip-tint`, `--eye-brightening` | Lip color and sclera brightness, 0..1 |
| `--glasses` | `off`, `round`, `square`, `sunglasses` |
| `--glasses-size` | 0.6..1.6; default 1 |
| `--particles` | `off`, `sparkle`, `sakura`, `both` |
| `--particle-count` | 0..200 total; default 40 |
| `--particle-size` | 0.25..3; default 1 |
| `--particle-speed` | 0..3; default 1; 0 freezes motion |

Explicit options override presets regardless of order. K-Beauty defaults:
smoothing .80, light .35, skin brightness .50, rosy .35, glow .40, lips .35,
eyes .40. Glasses and particles default off. Youth uses smoothing .75, light .45,
brightness .45. Natural uses smoothing .55 and light .25, other beauty controls 0.

K-Beauty, glasses, and panel mode load local face/landmark models. With tracking,
no detected face means no beauty. Lip/eye/glasses effects require valid landmarks;
status distinguishes face detection from landmarks ready. The panel keeps models
loaded so enabling effects does not restart capture. `--test-source` alone in the
panel skips models; use `--preset kbeauty` if testing with models loaded.

## Performance and limits

Use release builds. Begin at 640x480; camera hardware may deliver less than the
requested FPS. Check supported formats before adding `--mjpeg`:

```bash
v4l2-ctl -d /dev/video0 --list-formats-ext
```

Terminal statistics report pipeline FPS and foreground effect time, not end-to-end
Chrome latency. Inference runs on a bounded background queue; stale detections
expire. Only the largest face is tracked. Fast movement, profile views, occlusion,
lighting, and physical glasses can affect tracking/alignment.

Beauty is cosmetic smoothing and tinting, not age reconstruction. Skin masks can
miss pixels or show face/neck brightness differences. Eyes are not enlarged and
iris color is preserved. Glasses are 2D overlays without full 3D occlusion.
Particles avoid the detected face box, not the entire person; they can cover
clothing. No person segmentation or claim of Snapchat/Instagram parity.

## If virtual output fails

For `VIDIOC_G_FMT: Invalid argument` or output broken pipe, close Joscam, OBS,
and camera tabs. Inspect device ownership and capabilities:

```bash
sudo fuser -v /dev/video2
v4l2-ctl -d /dev/video2 --all
```

If the loopback module is stuck, with device users closed:

```bash
sudo modprobe -r v4l2loopback
sudo modprobe v4l2loopback devices=1 video_nr=2 exclusive_caps=1 card_label="Joscam Rust"
```

Restart Joscam/output, then Chrome. A listed device node alone does not establish
that it is usable. The app never runs sudo or changes system configuration.

If `as_encoded_bytes` is missing or Cargo cannot parse the lockfile, use the
explicit Rust 1.85.1 commands above. Cargo.lock intentionally uses format 3.

## Recording and checks

```bash
cargo +1.85.1 test --release --locked
cargo +1.85.1 run --release --locked -- --test-source --frames 30 --record smoke.mp4
ffprobe -v error -show_entries stream=width,height,nb_frames smoke.mp4
```

Recording replaces virtual output and refuses to overwrite existing files. A test
pattern checks transport, not beauty quality. Ordinary tests need no webcam or
models; optional model checks require setup and a 640x436 RGB24 face fixture:

```bash
JOSCAM_TEST_MESH=models/face_mesh_Nx3x192x192.onnx \
JOSCAM_TEST_MODEL=models/seeta_fd_frontal_v1.0.bin \
JOSCAM_TEST_RGB=/absolute/path/face.rgb \
cargo +1.85.1 test --release --locked real_ -- --ignored --nocapture
```

## Licensing

Rust source: MIT. FFmpeg, v4l2loopback, ONNX Runtime, and downloaded models have
their own licenses. Model sources/digests are in `models/README.md`; notices are
in `third-party/`. Review obligations before bundling a commercial installer.

## Face and lip reshape

Live panel sliders: **Face Slim** (0..1), **Chin** (-1..1, shorter to longer),
and **Lip Fullness** (-1..1, thinner to fuller). All default to 0 and presets
leave them neutral. CLI example:

```bash
cargo +1.85.1 run --release --locked -- --panel --preset kbeauty \
  --face-slim 0.35 --chin -0.15 --lip-fullness 0.3
```

Reshape requires valid local landmarks and automatically selects the existing
models. It applies bounded, feathered inverse warps after beauty/makeup and
before glasses/particles, so lip color follows reshaped lips. Lip warps avoid
sampling the detected inner mouth. No mesh means no reshape; malformed or strongly
asymmetric geometry is skipped. Local anchor smoothing reduces small jumps but
is not full optical-flow stabilization. Strong motion, open-mouth expressions,
and profiles still require visual testing. Background/hair near the jaw may bend;
start with small values. These are 2D cosmetic warps, not anatomical or 3D edits.
