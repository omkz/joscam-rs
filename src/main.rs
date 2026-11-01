mod beauty;
use std::{
    env,
    error::Error,
    io::{self, Read, Write},
    process::{Child, Command, Stdio},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Managed(Child);
impl Drop for Managed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Config {
    input: String,
    output: String,
    record: Option<String>,
    width: usize,
    height: usize,
    fps: u32,
    strength: f32,
    light: f32,
    frames: Option<usize>,
    test: bool,
    mjpeg: bool,
    preview: bool,
}
fn parse(args: impl Iterator<Item = String>) -> Result<Option<Config>> {
    let mut c = Config {
        input: "/dev/video0".into(),
        output: "/dev/video2".into(),
        record: None,
        width: 640,
        height: 480,
        fps: 30,
        strength: 0.55,
        light: 0.25,
        frames: None,
        test: false,
        mjpeg: false,
        preview: false,
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("Joscam Rust\n--input /dev/video0 --output /dev/video2\n--width 640 --height 480 --fps 30 --mjpeg\n--strength 0.55 --light 0.25 (0..1)\n--record demo.mp4 (instead of virtual camera)\n--preview (local window instead of virtual camera; Q/Esc closes)\n--test-source --frames 60\nCtrl+C stops. Choose Joscam in Chrome camera settings.");
            return Ok(None);
        }
        if arg == "--test-source" {
            c.test = true;
            continue;
        }
        if arg == "--preview" {
            c.preview = true;
            continue;
        }
        if arg == "--mjpeg" {
            c.mjpeg = true;
            continue;
        }
        let v = args.next().ok_or("Missing option value (use --help)")?;
        match arg.as_str() {
            "--input" => c.input = v,
            "--output" => c.output = v,
            "--record" => c.record = Some(v),
            "--width" => c.width = v.parse()?,
            "--height" => c.height = v.parse()?,
            "--fps" => c.fps = v.parse()?,
            "--strength" => c.strength = v.parse()?,
            "--light" => c.light = v.parse()?,
            "--frames" => c.frames = Some(v.parse()?),
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    if !(4..=1920).contains(&c.width)
        || !(4..=1080).contains(&c.height)
        || c.width % 2 != 0
        || c.height % 2 != 0
    {
        return Err("Dimensions must be even, width 4..1920 and height 4..1080".into());
    }
    if !(1..=60).contains(&c.fps)
        || !c.strength.is_finite()
        || !c.light.is_finite()
        || !(0.0..=1.0).contains(&c.strength)
        || !(0.0..=1.0).contains(&c.light)
        || c.frames == Some(0)
    {
        return Err("Invalid fps, strength, light, or frame limit".into());
    }
    if c.preview && c.record.is_some() {
        return Err("--preview and --record cannot be combined".into());
    }
    if !c.preview && c.record.is_none() && c.input == c.output {
        return Err("Input and virtual output must differ".into());
    }
    Ok(Some(c))
}
// Preview consumes the processed RGB stream directly, without opening a
// loopback output. The same beauty pipeline is used in either mode.
fn output_command(c: &Config) -> Command {
    let size = format!("{}x{}", c.width, c.height);
    let fps = c.fps.to_string();
    if c.preview {
        let mut command = Command::new("ffplay");
        command.args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-autoexit",
            "-window_title",
            "Joscam Rust - Beauty Preview (Q / Esc to close)",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            &size,
            "-framerate",
            &fps,
            "-probesize",
            "32",
            "-analyzeduration",
            "0",
            "-sync",
            "video",
            "-framedrop",
            "-i",
            "pipe:0",
        ]);
        return command;
    }
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-loglevel",
        "warning",
        "-nostdin",
        "-f",
        "rawvideo",
        "-pixel_format",
        "rgb24",
        "-video_size",
        &size,
        "-framerate",
        &fps,
        "-i",
        "pipe:0",
        "-an",
        "-threads",
        "1",
    ]);
    if let Some(path) = &c.record {
        command.args([
            "-n", "-c:v", "mpeg4", "-q:v", "3", "-pix_fmt", "yuv420p", path,
        ]);
    } else {
        command.args([
            "-c:v", "rawvideo", "-pix_fmt", "yuyv422", "-f", "v4l2", &c.output,
        ]);
    }
    command
}

fn run(c: Config) -> Result<()> {
    let size = format!("{}x{}", c.width, c.height);
    let fps = c.fps.to_string();
    let mut capture = Command::new("ffmpeg");
    capture.args(["-hide_banner", "-loglevel", "warning", "-nostdin"]);
    if c.test {
        capture.args([
            "-re",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size={size}:rate={fps}"),
        ]);
    } else {
        capture.args(["-f", "v4l2", "-framerate", &fps, "-video_size", &size]);
        if c.mjpeg {
            capture.args(["-input_format", "mjpeg"]);
        }
        capture.args(["-i", &c.input]);
    }
    capture.args([
        "-an",
        "-vf",
        &format!("scale={}:{}", c.width, c.height),
        "-threads",
        "1",
        "-pix_fmt",
        "rgb24",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    let mut capture = Managed(
        capture
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()?,
    );
    let mut source = capture.0.stdout.take().ok_or("No capture pipe")?;
    let mut sink = output_command(&c);
    let mut sink = Managed(sink.stdin(Stdio::piped()).spawn()?);
    let mut target = sink.0.stdin.take().ok_or("No output pipe")?;
    let mut src = vec![0; c.width * c.height * 3];
    let mut dst = src.clone();
    let mut count = 0;
    let mut beauty = beauty::Beauty::default();
    let mut processing_ms = 0.0;
    let mut report_at = std::time::Instant::now();
    let mut report_frames = 0;
    eprintln!("Starting {}x{} at requested {} FPS. Beauty is a centered skin-color heuristic; keep your face centered.",c.width,c.height,c.fps);
    loop {
        if let Some(status) = sink.0.try_wait()? {
            if c.preview && status.success() {
                eprintln!("Preview closed.");
                return Ok(());
            }
            return Err(format!("Output exited: {status}").into());
        }
        match source.read_exact(&mut src) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(
                    "Capture ended before requested stop; inspect FFmpeg message above".into(),
                )
            }
            Err(e) => return Err(e.into()),
        }
        let started = std::time::Instant::now();
        beauty.process(&src, &mut dst, c.width, c.height, c.strength, c.light);
        processing_ms += started.elapsed().as_secs_f64() * 1000.0;
        report_frames += 1;
        if let Err(error) = target.write_all(&dst) {
            if c.preview && error.kind() == io::ErrorKind::BrokenPipe {
                drop(target);
                let status = sink.0.wait()?;
                if status.success() {
                    eprintln!("Preview closed.");
                    return Ok(());
                }
                return Err(
                    format!("Preview exited: {status}; check FFplay/display errors above").into(),
                );
            }
            return Err(error.into());
        }
        count += 1;
        if report_at.elapsed().as_secs_f64() >= 2.0 {
            eprintln!(
                "Video {:.1} FPS | beauty {:.2} ms/frame",
                report_frames as f64 / report_at.elapsed().as_secs_f64(),
                processing_ms / report_frames as f64
            );
            processing_ms = 0.0;
            report_frames = 0;
            report_at = std::time::Instant::now();
        }
        if c.frames.is_some_and(|n| count >= n) {
            break;
        }
    }
    drop(target);
    let status = sink.0.wait()?;
    if !status.success() {
        return Err(format!("Output failed: {status}").into());
    }
    eprintln!("Processed {count} frames.");
    Ok(())
}
fn main() {
    let result = parse(env::args().skip(1)).and_then(|c| match c {
        Some(c) => run(c),
        None => Ok(()),
    });
    if let Err(e) = result {
        eprintln!("Joscam: {e}\nFor virtual output, check v4l2loopback, permissions, and whether another app owns the device.");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bad_config_rejected() {
        for args in [
            vec!["--width", "641"],
            vec!["--strength", "NaN"],
            vec!["--input", "/dev/video2"],
            vec!["--fps", "0"],
            vec!["--frames", "0"],
        ] {
            assert!(parse(args.into_iter().map(String::from)).is_err());
        }
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    fn config(args: &[&str]) -> Config {
        parse(args.iter().map(|s| s.to_string())).unwrap().unwrap()
    }
    #[test]
    fn preview_is_standalone_and_uses_processed_rgb() {
        let c = config(&["--preview", "--input", "/dev/video2"]);
        assert!(c.preview);
        let command = output_command(&c);
        assert_eq!(command.get_program(), "ffplay");
        let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"rgb24"));
        assert!(args.contains(&"pipe:0"));
        assert!(!args.contains(&"v4l2"));
        assert!(!args.contains(&"/dev/video2"));
    }
    #[test]
    fn preview_and_record_are_mutually_exclusive() {
        assert!(parse(
            ["--preview", "--record", "a.mp4"]
                .into_iter()
                .map(String::from)
        )
        .is_err());
    }
    #[test]
    fn virtual_output_is_still_default() {
        let c = config(&[]);
        assert!(!c.preview);
        let command = output_command(&c);
        assert_eq!(command.get_program(), "ffmpeg");
        assert!(command.get_args().any(|s| s == "/dev/video2"));
    }
}
