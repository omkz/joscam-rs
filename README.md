# Joscam Rust — initial prototype

A new Linux camera project. Rust processes RGB frames locally; FFmpeg handles
webcam decoding and V4L2 output. This is not a browser extension and not yet a
finished product. There is no cloud service or upload of camera frames.

## Linux Mint setup

Install Rust/Cargo using your preferred trusted installer, then:

```bash
sudo apt install ffmpeg v4l-utils v4l2loopback-dkms linux-headers-$(uname -r)
sudo modprobe v4l2loopback devices=1 video_nr=2 exclusive_caps=1 card_label="Joscam Rust"
v4l2-ctl --list-devices
cargo test
cargo build --release
./target/release/joscam-rs --input /dev/video0 --output /dev/video2
```

The output device must be a loopback device, never your physical webcam. Stop
Python Joscam or OBS first if they use the same devices. Close with Ctrl+C.

Start this app before opening Chrome's camera settings. Select `Joscam Rust`
(or your existing loopback label, e.g. `Joscam`). Grant the website camera access.
A website may reject virtual cameras; Chrome compatibility is not a guarantee
of acceptance by OmeTV. No site-specific detection bypass is implemented.

## Controls

```bash
./target/release/joscam-rs --strength 0.65 --light 0.3
./target/release/joscam-rs --width 1280 --height 720 --fps 30 --mjpeg
```

Start at 640x480. Requested FPS depends on camera formats and processing speed;
Rust alone does not guarantee 30 FPS. Check supported capture formats with:

```bash
v4l2-ctl -d /dev/video0 --list-formats-ext
```

`--mjpeg` is optional; only use it if your camera supports MJPEG at the requested
resolution/rate. Audio continues to come from the microphone selected in Chrome.

## Test without a webcam

```bash
cargo run --release -- --test-source --frames 30 --record smoke.mp4
ffprobe -v error -show_entries stream=width,height,nb_frames smoke.mp4
```

Recording refuses to overwrite an existing file. `--record` replaces virtual
output; a test pattern validates video transport, not beauty quality. Inspect a
webcam recording with `--frames 90 --record portrait.mp4` to evaluate actual skin.

## Beauty limits

The initial filter uses a feathered, centered portrait ellipse plus a skin-color
heuristic and edge-aware smoothing. Keep your face centered. It has **no face
landmarks or tracking**, so skin-colored objects can also be affected. Results
vary with lighting and skin tone. It does not reconstruct faces or change age.
The next quality milestone is tracked face/skin masking and temporal stability,
then an interactive preview UI. Current controls are command-line only.

## If virtual output fails

Close other producers and camera tabs. Inspect:

```bash
v4l2-ctl -d /dev/video2 --all
sudo fuser -v /dev/video2
```

A loopback device needs `Video Output` capability before a producer connects.
If the module is stuck, close users of the device, unload and reload it:

```bash
sudo modprobe -r v4l2loopback
sudo modprobe v4l2loopback devices=1 video_nr=2 exclusive_caps=1 card_label="Joscam Rust"
```

This app never runs sudo or changes your system configuration automatically.

## Licensing

Rust source: MIT (see LICENSE). FFmpeg and v4l2loopback are separately installed
third-party programs with their own licenses. Reassess distribution obligations
before bundling dependencies into a commercial installer.

## Beauty performance

Beauty uses a reusable two-pass running-sum blur and an edge/detail guard,
replacing the per-pixel 25-neighbor bilateral approximation. The visual result
is not identical to the original filter. The centered skin-color mask and its
limitations still apply. Face Light alone skips smoothing entirely.

Every two seconds the terminal reports delivered pipeline FPS and mean beauty
processing milliseconds per frame. This measures the local pipeline, not
Chrome rendering or end-to-end latency. At 30 FPS the whole pipeline needs to
fit within about 33 ms/frame; leave room for capture, conversion, and Chrome.

Run the synthetic processing benchmark (not a camera or visual-quality test):

```bash
cargo test --release benchmark_beauty -- --ignored --nocapture
```

Compare enabled beauty against `--strength 0 --light 0` on the same camera.
Use `--release` for real-time use. Webcam quality and actual FPS must still be
verified on your machine.

## Local beauty preview

Close any other application using your physical webcam, then run:

```bash
cargo run --release -- --preview --input /dev/video0 --strength 0.65 --light 0.3
```

A local window shows the processed video. Press **Q**, **Esc**, or close the
window to stop the preview and release the camera. This standalone mode does
not need v4l2loopback, Chrome, or OmeTV, and does not send video to the virtual
camera. Remove `--preview` to use virtual output as before. It cannot be
combined with `--record`. FFplay is supplied by Linux Mint's `ffmpeg` package;
preview requires an active graphical desktop session. Strength and light are
still command-line settings, not live sliders.

Preview without physical hardware:

```bash
cargo run --release -- --preview --test-source --frames 90
```

## Brighter skin / Youth preset

```bash
cargo run --release -- --preview --preset youth
cargo run --release -- --preview --preset youth --skin-brightening 0.65
```

`--skin-brightening` ranges from 0 (off, default) to 1 (strongest). It blends
skin candidates toward white inside the existing portrait mask, gently reducing
saturation as it brightens. It does not brighten the whole frame. Skin-colored
background objects within the mask may still be affected.

`--preset youth` sets smoothing to 0.75, face light to 0.45 and skin brightening
to 0.45. Explicit `--strength`, `--light`, and `--skin-brightening` values override
the preset regardless of argument order. The default `natural` preset preserves
previous defaults. Set all three controls to zero for exact passthrough.

Youth is cosmetic smoothing/lighting, not age reconstruction or wrinkle-specific
removal. There is still no face detection, landmark tracking or skin segmentation.
For Chrome, omit `--preview` and use your usual virtual output device.

## K-Beauty with local face detection

One-time setup (requires Git and sha256sum):

```bash
bash scripts/download-face-model.sh
cargo run --release -- --preview --preset kbeauty
```

K-Beauty adds soft skin brightening, a subtle rosy tint, and a gentle glow.
It loads the local SeetaFace model through the Rust `rustface` library. The
small grayscale detection input is processed on a background thread at up to
10 detection requests/second. Only the largest detected face is followed.
The video pipeline never waits for inference; queued work is bounded and
old results expire after 350 ms. This can create a short delay when moving
quickly. No detected face means **no cosmetic effects** in tracking mode.
Terminal status reads `face detected` or `face searching`.

```bash
cargo run --release -- --preview --preset kbeauty --rosy 0.25 --glow 0.5 --skin-brightening 0.45
```

`--rosy` and `--glow` each range from 0 to 1 and default to 0 outside the
K-Beauty preset. Explicit values override preset defaults in either order.
K-Beauty defaults: strength 0.70, face light 0.35, skin brightening 0.35,
rosy 0.35, glow 0.40. Use `--face-model PATH` to enable tracking with another
preset; default K-Beauty path is `models/seeta_fd_frontal_v1.0.bin` relative
to the working directory. Run from the project root or supply an absolute path.
Omit `--preview` for virtual output to Chrome as before.

This is **face-box detection with position smoothing, not facial landmarks**.
Eye/mouth protection uses approximate box-relative regions; it is not precise
makeup placement. No lip tint, blush, eye enlargement or jaw reshaping is
implemented. Strong profiles, occlusions, dim lighting, and skin-color mask
limitations can cause missed detections, false detections or visible switching.
No generative age/identity transformation is performed. Evaluate the look and
performance on your webcam; it is not a claim of Instagram/Snapchat parity.

The older centered-mask description above applies when no face model is used.
Downloading the pinned, checksum-verified model is the only model setup;
inference remains local. The model is not committed to this repository.
Upstream model/library attribution is in `models/README.md` and
`third-party/rustface-LICENSE.txt`. The crate's default Rayon feature is disabled
to avoid creating an additional inference thread pool.

Optional real-model regression check: provide a 640x436 RGB24 raw fixture
containing a sufficiently large face (not committed here):

```bash
JOSCAM_TEST_MODEL=models/seeta_fd_frontal_v1.0.bin \
JOSCAM_TEST_RGB=/absolute/path/face.rgb \
cargo test --release --locked real_model_detects -- --ignored --nocapture
```

This checks detection followed by clearing the mask on blank input. Normal
`cargo test --locked` does not require a model download or personal image.

## Dense landmarks, lip tint, and brighter eyes (PR #4 update)

The latest K-Beauty path now uses 468 facial landmarks in addition to face-box
detection. Download the extra, checksum-verified ONNX model once:

```bash
bash scripts/setup-beauty.sh
cargo run --release --locked -- --preview --preset kbeauty --compare
```

The left half of the comparison shows the original frame and the right half
shows the processed frame. Remove `--compare` for a normal preview; comparison
is only allowed with `--preview`.

The updated path uses the face contour for skin coverage, protects actual eye
and lip contours from smoothing, and excludes the inner mouth from lip tint.
Eyebrows are excluded as well. Skin brightening and face light are stronger, and the smoothing detail guard
allows more texture reduction. The skin-color gate is broader inside a validated
face mesh, so neutral skin is not silently dropped. This is still cosmetic
retouching, not age reconstruction or skin-condition diagnosis.

```bash
cargo run --release --locked -- --preview --preset kbeauty \
  --skin-brightening 0.65 --strength 0.85 \
  --lip-tint 0.5 --eye-brightening 0.5
```

`--lip-tint` blends a luminance-adjusted rose tint inside the outer lip contour,
excluding the mouth cavity. `--eye-brightening` selectively lifts bright,
low-saturation pixels inside the eye contours; dark pupils are preserved. It
does not enlarge eyes, change iris color, or reshape the face. Each control is
0..1 and can be disabled with 0. K-Beauty defaults now use smoothing 0.80,
skin brightening 0.50, lip tint 0.35 and eye brightening 0.40; face light/rosy/glow
remain 0.35/0.35/0.40. Explicit control values override preset defaults.

Terminal output distinguishes `face detected` from `landmarks ready`. Lip and
eye effects require valid landmarks and never fall back to guessed positions.
If a face box exists but the mesh is invalid, skin retouch uses the box mask
while feature effects stop. No face means no effects. Contours are refreshed
on the background inference path, so rapid movement, blinking, open-mouth
motion, strong head tilt and occlusion still require visual testing. The model
crop currently has no eye-line roll correction.

Rust calls ONNX Runtime 1.19.2 locally through pinned `ort` bindings. The setup
script downloads both neural models and the CPU runtime, checks their SHA-256
digests, and preserves the runtime's license. Automatic runtime installation
currently supports Linux x86_64. No Python or cloud inference is involved.
Runtime files live in `models/onnxruntime/`; set `ORT_DYLIB_PATH` to a compatible
ONNX Runtime API 19 library if using a custom installation. Run from the project
root unless supplying absolute model/runtime paths.

The first Rust compilation adds dependencies. Run release builds for video and
use the committed lockfile (`--locked`). Cargo.lock remains format 3; selected
bindings/dependencies avoid requiring a very recent toolchain. Validation uses
Rust 1.85.1 and 1.99.1; a Rust 1.75 build has not been verified.

The earlier box-only limits above describe the fallback path. The ONNX export
comes from Yakhyokhuja Valikhujaev's Apache-2.0 MediaPipe Face Mesh port; source,
weight digest, and notices are in `models/README.md` and `third-party/`.
Model bytes are not committed and camera frames are never uploaded.

To run both real-model regression checks with a local 640x436 RGB24 face fixture:

```bash
JOSCAM_TEST_MESH=models/face_mesh_Nx3x192x192.onnx \
JOSCAM_TEST_MODEL=models/seeta_fd_frontal_v1.0.bin \
JOSCAM_TEST_RGB=/absolute/path/face.rgb \
cargo test --release --locked real_ -- --ignored --nocapture
```

These checks require runtime setup. The ordinary test suite requires neither
model downloads nor a camera. Missing runtime/model errors occur before opening
the webcam. Strong brightening can still create a visible face/neck difference;
use comparison mode and reduce `--skin-brightening` when desired.

## Sparkle and sakura overlays

```bash
cargo run --release --locked -- --preview --particles both
cargo run --release --locked -- --preview --preset kbeauty --particles sakura \
  --particle-count 60 --particle-size 1.2 --particle-speed 0.8
```

`--particles` accepts `off` (default), `sparkle`, `sakura`, or `both`. Particle
count is the total number of candidates, shared between styles in `both` mode:

| Control | Default | Range |
| --- | --- | --- |
| `--particle-count` | 40 | 0–200 |
| `--particle-size` | 1.0 | 0.25–3.0 |
| `--particle-speed` | 1.0 | 0–3.0; 0 freezes animation |

These are procedural screen overlays with no image assets or cloud services.
They are not background segmentation: particles may appear over your clothing
or other foreground objects. With face tracking enabled, particles intersecting
the detected face box are skipped, including a small safety margin. This is
best-effort face avoidance; without a detection they can appear anywhere.
Skipped particles are not replaced, so the visible count may be lower.

Animation follows elapsed time, not frame count. Sprites are clipped to the
frame, and the renderer reuses the output buffer without allocating per frame.
Large counts/sizes cost more CPU; start with 40–60 particles. Rendering occurs
after beauty and before the comparison split, so `--compare` keeps the original
left half free of particles. For Chrome, remove `--preview` (and `--compare`);
the same overlay is sent to the virtual camera. Particles alone do not require
neural models or ONNX Runtime. Beauty presets retain their usual model setup.

## Virtual glasses

```bash
cargo +1.85.1 run --release --locked -- --preview --preset kbeauty --glasses round
cargo +1.85.1 run --release --locked -- --preview --glasses square --glasses-size 1.1
```

`--glasses` accepts `off` (default), `round`, or `square`. `--glasses-size` is
0.6–1.6, default 1.0. Both styles are dark procedural frames with transparent
lens interiors. They follow the centers/tilt of the eye landmarks and include a
bridge and short temple arms. No extra model or image asset is downloaded.
If not using K-Beauty, enabling glasses automatically enables the existing face
and landmark model paths. Run `bash scripts/setup-beauty.sh` if not already set up.

Glasses draw after beauty/particles and before the original/processed preview
split. Remove `--preview` and `--compare` to send them to Chrome through the
virtual camera. They can be combined with `--particles both`. Fresh valid
landmarks are required; no landmark result means no glasses. This is a 2D
overlay, not a fitted 3D object, so strong yaw, partial occlusion and rapid motion
can reduce alignment. It does not replace or remove physical glasses.
