//! Procedural screen overlays, not person/background segmentation.
use crate::face::FaceRect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Off,
    Sparkle,
    Sakura,
    Both,
}
impl Mode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "off" => Ok(Self::Off),
            "sparkle" => Ok(Self::Sparkle),
            "sakura" => Ok(Self::Sakura),
            "both" => Ok(Self::Both),
            _ => Err("Particles must be off, sparkle, sakura, or both".into()),
        }
    }
}
#[derive(Clone, Copy)]
pub struct Settings {
    pub mode: Mode,
    pub count: usize,
    pub size: f32,
    pub speed: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            count: 40,
            size: 1.0,
            speed: 1.0,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.count > 200
            || !self.size.is_finite()
            || !(0.25..=3.0).contains(&self.size)
            || !self.speed.is_finite()
            || !(0.0..=3.0).contains(&self.speed)
        {
            return Err("Particles: count 0..200, size 0.25..3, speed 0..3".into());
        }
        Ok(())
    }
}
fn random(index: usize, salt: u32) -> f32 {
    let mut n = (index as u32)
        .wrapping_add(salt)
        .wrapping_mul(747796405)
        .wrapping_add(2891336453);
    n = ((n >> ((n >> 28) + 4)) ^ n).wrapping_mul(277803737);
    n = (n >> 22) ^ n;
    (n & 0x00ff_ffff) as f32 / 16777216.0
}
fn intersects_face(
    x: f32,
    y: f32,
    r: f32,
    width: usize,
    height: usize,
    face: Option<FaceRect>,
) -> bool {
    if let Some(f) = face {
        let left = (f.x - f.w * 0.08) * width as f32;
        let top = (f.y - f.h * 0.10) * height as f32;
        let right = (f.x + f.w * 1.08) * width as f32;
        let bottom = (f.y + f.h * 1.08) * height as f32;
        x + r >= left && x - r <= right && y + r >= top && y - r <= bottom
    } else {
        false
    }
}
/// Render from elapsed wall time, independently of capture FPS. No allocations
/// or random state in the per-frame path. Face avoidance is box-based only.
pub fn render(
    frame: &mut [u8],
    width: usize,
    height: usize,
    elapsed: f64,
    settings: Settings,
    face: Option<FaceRect>,
) {
    assert_eq!(frame.len(), width * height * 3);
    if settings.mode == Mode::Off || settings.count == 0 || width == 0 || height == 0 {
        return;
    }
    // Use f64 time and reduce each angle separately: no global hourly jump.
    let t = elapsed.max(0.0) * settings.speed as f64;
    let wave = |phase: f32, rate: f32| {
        ((phase as f64 + t * rate as f64).rem_euclid(std::f64::consts::TAU)).sin() as f32
    };
    for i in 0..settings.count {
        let petal = settings.mode == Mode::Sakura || (settings.mode == Mode::Both && i % 2 == 0);
        let phase = random(i, 31) * std::f32::consts::TAU;
        let (x, y, r, angle, opacity) = if petal {
            let fall =
                (random(i, 73) as f64 + t * (0.035 + random(i, 12) as f64 * 0.035)).fract() as f32;
            let px = (random(i, 7) as f64 + t * 0.012 + wave(phase, 0.7) as f64 * 0.035)
                .rem_euclid(1.0) as f32;
            (
                px * width as f32,
                fall * height as f32,
                (settings.size * (4.0 + random(i, 9) * 4.0) * height as f32 / 480.0)
                    .clamp(1.0, 32.0),
                (phase as f64 + t * (0.5 + random(i, 4) as f64)).rem_euclid(std::f64::consts::TAU)
                    as f32,
                0.50 + random(i, 8) * 0.30,
            )
        } else {
            let px = (random(i, 99) + wave(phase, 0.23) * 0.025).rem_euclid(1.0);
            let py = (random(i, 55) + wave(phase + std::f32::consts::FRAC_PI_2, 0.18) * 0.025)
                .rem_euclid(1.0);
            (
                px * width as f32,
                py * height as f32,
                (settings.size * (3.0 + random(i, 9) * 4.0) * height as f32 / 480.0)
                    .clamp(1.0, 32.0),
                0.0,
                0.25 + 0.65 * (0.5 + 0.5 * wave(phase, 1.2 + random(i, 20))).powi(2),
            )
        };
        if intersects_face(x, y, r, width, height, face) {
            continue;
        }
        let x0 = (x - r - 1.0).floor().max(0.0) as usize;
        let y0 = (y - r - 1.0).floor().max(0.0) as usize;
        let x1 = (x + r + 1.0).ceil().min(width as f32) as usize;
        let y1 = (y + r + 1.0).ceil().min(height as f32) as usize;
        let (sn, cs) = angle.sin_cos();
        let color = if petal {
            [255.0, 165.0 + random(i, 19) * 35.0, 205.0]
        } else {
            [255.0, 243.0, 208.0]
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                let dx = (xx as f32 + 0.5 - x) / r;
                let dy = (yy as f32 + 0.5 - y) / r;
                let alpha = if petal {
                    let u = dx * cs + dy * sn;
                    let v = (-dx * sn + dy * cs) / 0.55;
                    let body = (1.0 - u * u - v * v).clamp(0.0, 1.0);
                    let notch = ((u - 0.72).powi(2) + (v * 0.75).powi(2)).sqrt();
                    (body * 5.0).clamp(0.0, 1.0) * (notch / 0.22).clamp(0.0, 1.0) * opacity
                } else {
                    let horizontal = (1.0 - dx.abs()).max(0.0) * (1.0 - dy.abs() / 0.13).max(0.0);
                    let vertical = (1.0 - dy.abs()).max(0.0) * (1.0 - dx.abs() / 0.13).max(0.0);
                    let glow = (1.0 - dx * dx - dy * dy).max(0.0).powi(3) * 0.18;
                    (horizontal.max(vertical) + glow).min(1.0) * opacity
                };
                if alpha <= 0.0 {
                    continue;
                }
                let pos = (yy * width + xx) * 3;
                for c in 0..3 {
                    frame[pos + c] =
                        (frame[pos + c] as f32 * (1.0 - alpha) + color[c] * alpha).round() as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn off_and_zero_count_preserve_frame() {
        let original = vec![100; 64 * 48 * 3];
        let mut frame = original.clone();
        render(&mut frame, 64, 48, 2.0, Settings::default(), None);
        assert_eq!(frame, original);
        render(
            &mut frame,
            64,
            48,
            2.0,
            Settings {
                mode: Mode::Both,
                count: 0,
                ..Settings::default()
            },
            None,
        );
        assert_eq!(frame, original);
    }
    #[test]
    fn modes_are_deterministic_and_animate() {
        for mode in [Mode::Sparkle, Mode::Sakura, Mode::Both] {
            let mut a = vec![0; 320 * 240 * 3];
            let mut b = a.clone();
            let mut c = a.clone();
            let settings = Settings {
                mode,
                count: 40,
                ..Settings::default()
            };
            render(&mut a, 320, 240, 1.0, settings, None);
            render(&mut b, 320, 240, 1.0, settings, None);
            render(&mut c, 320, 240, 2.0, settings, None);
            assert_eq!(a, b);
            assert!(a.iter().any(|&v| v > 0));
            assert_ne!(a, c);
        }
    }
    #[test]
    fn stopped_motion_and_face_avoidance() {
        let settings = Settings {
            mode: Mode::Both,
            speed: 0.0,
            ..Settings::default()
        };
        let mut a = vec![0; 320 * 240 * 3];
        let mut b = a.clone();
        render(&mut a, 320, 240, 0.0, settings, None);
        render(&mut b, 320, 240, 50.0, settings, None);
        assert_eq!(a, b);
        let mut frame = vec![0; 320 * 240 * 3];
        render(
            &mut frame,
            320,
            240,
            1.0,
            Settings {
                mode: Mode::Both,
                count: 200,
                ..Settings::default()
            },
            Some(FaceRect {
                x: 0.25,
                y: 0.25,
                w: 0.5,
                h: 0.5,
            }),
        );
        for y in 60..180 {
            for x in 80..240 {
                assert_eq!(&frame[(y * 320 + x) * 3..(y * 320 + x) * 3 + 3], &[0; 3]);
            }
        }
    }
    #[test]
    fn validation_and_small_frames() {
        assert!(Mode::parse("rainbow").is_err());
        for settings in [
            Settings {
                count: 201,
                ..Settings::default()
            },
            Settings {
                size: f32::NAN,
                ..Settings::default()
            },
            Settings {
                speed: -1.0,
                ..Settings::default()
            },
        ] {
            assert!(settings.validate().is_err());
        }
        render(
            &mut [0; 12],
            2,
            2,
            1.0,
            Settings {
                mode: Mode::Both,
                ..Settings::default()
            },
            None,
        );
    }
}
