//! Loopback-only control surface. No camera permission or network assets needed.
use crate::{Config, Result};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
struct State {
    config: Config,
    enabled: bool,
    frame: Vec<u8>,
    status: String,
    face: bool,
    mesh: bool,
    published: Instant,
}
pub struct Panel {
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
impl Panel {
    pub fn start(config: Config) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?.to_string();
        let mut random = [0u8; 24];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
        let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let root = format!("/{token}");
        let url = format!("http://{address}{root}/");
        eprintln!("Control panel: {url}\nVirtual output starts OFF. Ctrl+C stops the app.");
        let state = Arc::new(Mutex::new(State {
            config,
            enabled: false,
            frame: Vec::new(),
            status: "Preview only".into(),
            face: false,
            mesh: false,
            published: Instant::now() - Duration::from_secs(1),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let stopped = stop.clone();
        thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = handle(&mut stream, &shared, &address, &root);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        // Detached browser launcher; the URL above also works when xdg-open is absent.
        if let Ok(mut child) = std::process::Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Ok(Self { state, stop })
    }
    pub fn settings(&self) -> (Config, bool) {
        let s = self.state.lock().unwrap();
        (s.config.clone(), s.enabled)
    }
    pub fn status(&self, message: &str) {
        self.state.lock().unwrap().status = message.into();
    }
    pub fn fail(&self, message: &str) {
        let mut s = self.state.lock().unwrap();
        s.enabled = false;
        s.status = message.into();
    }
    pub fn frame(&self, rgb: &[u8], w: usize, h: usize, face: bool, mesh: bool) {
        let mut s = self.state.lock().unwrap();
        if s.published.elapsed() < Duration::from_millis(100) {
            return;
        }
        s.frame = bmp(rgb, w, h);
        s.face = face;
        s.mesh = mesh;
        s.published = Instant::now();
        if s.enabled {
            s.status = "Virtual camera active".into();
        }
    }
}
fn bmp(rgb: &[u8], w: usize, h: usize) -> Vec<u8> {
    let width = w.min(320);
    let height = (h * width / w).max(1);
    let stride = (width * 3 + 3) & !3;
    let mut out = vec![0; 54 + stride * height];
    out[..2].copy_from_slice(b"BM");
    let length = out.len() as u32;
    out[2..6].copy_from_slice(&length.to_le_bytes());
    out[10..14].copy_from_slice(&54u32.to_le_bytes());
    out[14..18].copy_from_slice(&40u32.to_le_bytes());
    out[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    out[22..26].copy_from_slice(&(height as i32).to_le_bytes());
    out[26..28].copy_from_slice(&1u16.to_le_bytes());
    out[28..30].copy_from_slice(&24u16.to_le_bytes());
    for y in 0..height {
        for x in 0..width {
            let i = ((y * h / height) * w + x * w / width) * 3;
            let j = 54 + (height - 1 - y) * stride + x * 3;
            out[j..j + 3].copy_from_slice(&[rgb[i + 2], rgb[i + 1], rgb[i]]);
        }
    }
    out
}
fn settings(c: &Config) -> Vec<(&'static str, String)> {
    vec![
        ("filter", c.filter.style.name().into()),
        ("filter-amount", c.filter.amount.to_string()),
        ("face-slim", c.reshape.slim.to_string()),
        ("chin", c.reshape.chin.to_string()),
        ("lip-fullness", c.reshape.lips.to_string()),
        ("strength", c.strength.to_string()),
        ("light", c.light.to_string()),
        ("skin-brightening", c.skin_brightening.to_string()),
        ("rosy", c.rosy.to_string()),
        ("glow", c.glow.to_string()),
        ("lip-tint", c.lip_tint.to_string()),
        ("eye-brightening", c.eye_brightening.to_string()),
        ("glasses", format!("{:?}", c.glasses.style).to_lowercase()),
        ("glasses-size", c.glasses.size.to_string()),
        (
            "particles",
            format!("{:?}", c.particles.mode).to_lowercase(),
        ),
        ("particle-count", c.particles.count.to_string()),
        ("particle-size", c.particles.size.to_string()),
        ("particle-speed", c.particles.speed.to_string()),
    ]
}
fn update(c: &Config, body: &str) -> std::result::Result<Config, String> {
    let mut values = settings(c);
    for entry in body.split('&') {
        let (key, value) = entry.split_once('=').ok_or("Malformed settings")?;
        let target = values
            .iter_mut()
            .find(|(k, _)| *k == key)
            .ok_or("Unknown setting")?;
        target.1 = value.into();
    }
    let mut args = vec![
        "--width".into(),
        c.width.to_string(),
        "--height".into(),
        c.height.to_string(),
    ];
    if let Some(path) = &c.landmark_model {
        args.extend(["--landmark-model".into(), path.clone()]);
    }
    for (key, value) in values {
        args.extend([format!("--{key}"), value]);
    }
    let parsed = crate::parse(args.into_iter())
        .map_err(|e| e.to_string())?
        .ok_or("Missing settings")?;
    let mut next = c.clone();
    next.filter = parsed.filter;
    next.reshape = parsed.reshape;
    next.strength = parsed.strength;
    next.light = parsed.light;
    next.skin_brightening = parsed.skin_brightening;
    next.rosy = parsed.rosy;
    next.glow = parsed.glow;
    next.lip_tint = parsed.lip_tint;
    next.eye_brightening = parsed.eye_brightening;
    next.particles = parsed.particles;
    next.glasses = parsed.glasses;
    if c.landmark_model.is_none()
        && (next.reshape.active()
            || next.glasses.style != crate::glasses::Style::Off
            || next.lip_tint > 0.0
            || next.eye_brightening > 0.0)
    {
        return Err("Restart without --test-source to enable face effects".into());
    }
    Ok(next)
}
fn reply(stream: &mut TcpStream, code: &str, mime: &str, body: &[u8]) -> std::io::Result<()> {
    write!(stream,"HTTP/1.1 {code}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n",body.len())?;
    stream.write_all(body)
}
fn handle(
    stream: &mut TcpStream,
    shared: &Arc<Mutex<State>>,
    address: &str,
    root: &str,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut bytes = Vec::new();
    let mut chunk = [0; 2048];
    let (header_end, length) = loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > 16384 {
            return reply(
                stream,
                "413 Payload Too Large",
                "text/plain",
                b"Request too large",
            );
        }
        if let Some(i) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..i]);
            let length = headers
                .lines()
                .skip(1)
                .find_map(|l| {
                    l.split_once(':')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .map(|(_, v)| v.trim().parse::<usize>())
                })
                .transpose()
                .unwrap_or(Some(20000))
                .unwrap_or(0);
            break (i + 4, length);
        }
    };
    if length > 8192 {
        return reply(
            stream,
            "413 Payload Too Large",
            "text/plain",
            b"Request too large",
        );
    }
    while bytes.len() < header_end + length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let mut request = headers.lines().next().unwrap_or("").split_whitespace();
    let method = request.next().unwrap_or("");
    let path = request.next().unwrap_or("").split('?').next().unwrap_or("");
    let host = headers.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
            .map(|(_, v)| v.trim())
    });
    let origin = headers.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.eq_ignore_ascii_case("origin"))
            .map(|(_, v)| v.trim())
    });
    if host != Some(address) || origin.is_some_and(|o| o != format!("http://{address}")) {
        return reply(stream, "403 Forbidden", "text/plain", b"Local panel only");
    }
    let route = path.strip_prefix(root).unwrap_or("invalid");
    let (code, mime, data) = {
        let mut s = shared.lock().unwrap();
        match (method, route) {
            ("GET", "/") => (
                "200 OK",
                "text/html; charset=utf-8",
                include_bytes!("panel.html").to_vec(),
            ),
            ("GET", "/frame") => ("200 OK", "image/bmp", s.frame.clone()),
            ("GET", "/state") => {
                let mut text = format!(
                    "enabled={}\nstatus={}\nface={}\nlandmarks={}\n",
                    s.enabled, s.status, s.face, s.mesh
                );
                for (k, v) in settings(&s.config) {
                    text.push_str(&format!("{k}={v}\n"));
                }
                ("200 OK", "text/plain", text.into_bytes())
            }
            ("POST", "/start") => {
                s.enabled = true;
                s.status = "Starting virtual camera".into();
                ("200 OK", "text/plain", b"OK".to_vec())
            }
            ("POST", "/stop") => {
                s.enabled = false;
                s.status = "Preview only".into();
                ("200 OK", "text/plain", b"OK".to_vec())
            }
            ("POST", "/settings") => {
                match std::str::from_utf8(&bytes[header_end..header_end + length])
                    .map_err(|e| e.to_string())
                    .and_then(|b| update(&s.config, b))
                {
                    Ok(c) => {
                        s.config = c;
                        ("200 OK", "text/plain", b"OK".to_vec())
                    }
                    Err(e) => ("400 Bad Request", "text/plain", e.into_bytes()),
                }
            }
            _ => ("404 Not Found", "text/plain", b"Not found".to_vec()),
        }
    };
    reply(stream, code, mime, &data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_are_atomic_and_preserve_devices() {
        let c = crate::parse(
            ["--preset", "kbeauty", "--output", "/dev/video8"]
                .into_iter()
                .map(String::from),
        )
        .unwrap()
        .unwrap();
        let next = update(
            &c,
            "strength=0.9&glasses=sunglasses&face-slim=0.4&chin=-0.2&lip-fullness=0.3&filter=retro&filter-amount=0.7",
        )
        .unwrap();
        assert_eq!(next.output, "/dev/video8");
        assert_eq!(next.strength, 0.9);
        assert_eq!(next.filter.style, crate::filter::Style::Retro);
        assert_eq!(next.filter.amount, 0.7);
        assert_eq!(next.reshape.slim, 0.4);
        assert_eq!(next.reshape.chin, -0.2);
        assert_eq!(next.reshape.lips, 0.3);
        assert_eq!(next.glasses.style, crate::glasses::Style::Sunglasses);
        assert!(update(&c, "strength=NaN").is_err());
        assert!(update(&c, "output=evil").is_err());
        assert_eq!(c.strength, 0.8);
    }
    #[test]
    fn bitmap_has_correct_padding_orientation_and_colors() {
        let image = bmp(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255], 2, 2);
        assert_eq!(&image[..2], b"BM");
        assert_eq!(image.len(), 70);
        assert_eq!(&image[54..60], &[255, 0, 0, 255, 255, 255]);
        assert_eq!(&image[62..68], &[0, 0, 255, 0, 255, 0]);
    }
}

#[cfg(test)]
mod mode_tests {
    #[test]
    fn panel_keeps_output_separate_and_loads_models_for_live_controls() {
        let config = crate::parse(["--panel"].into_iter().map(String::from))
            .unwrap()
            .unwrap();
        assert!(config.panel);
        assert!(!config.preview);
        assert!(config.face_model.is_some());
        assert!(config.landmark_model.is_some());
        for option in ["--preview", "--compare"] {
            assert!(crate::parse(["--panel", option].into_iter().map(String::from)).is_err());
        }
        assert!(crate::parse(
            ["--panel", "--record", "test.mp4"]
                .into_iter()
                .map(String::from)
        )
        .is_err());
    }
}
