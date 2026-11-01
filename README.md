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
