//! Lightweight full-frame looks, independent of neural models.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    Off,
    Retro,
}
impl Style {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "off" => Ok(Self::Off),
            "retro" => Ok(Self::Retro),
            _ => Err("Filter must be off or retro".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Retro => "retro",
        }
    }
}
#[derive(Clone, Copy)]
pub struct Settings {
    pub style: Style,
    pub amount: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            style: Style::Off,
            amount: 0.65,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.amount.is_finite() || !(0.0..=1.0).contains(&self.amount) {
            Err("Filter amount must be 0..1".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Default)]
pub struct Filter {
    scratch: Vec<u8>,
    shading: Vec<i32>,
    dimensions: (usize, usize),
}
fn noise(x: usize, y: usize, t: u32) -> i32 {
    let mut n = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(t.wrapping_mul(2246822519));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((((n ^ (n >> 16)) & 255) as i32 - 128) * 9) >> 8
}
impl Filter {
    pub fn process(&mut self, rgb: &mut [u8], w: usize, h: usize, seconds: f64, s: Settings) {
        if s.style == Style::Off
            || s.amount == 0.0
            || s.validate().is_err()
            || w == 0
            || h == 0
            || rgb.len() != w * h * 3
            || !seconds.is_finite()
            || seconds < 0.0
        {
            return;
        }
        if self.dimensions != (w, h) {
            self.shading.resize(w * h, 0);
            for y in 0..h {
                let ny = 2.0 * y as f32 / h as f32 - 1.0;
                let scan = if y % 3 == 0 { 0.94 } else { 1.0 };
                for x in 0..w {
                    let nx = 2.0 * x as f32 / w as f32 - 1.0;
                    self.shading[y * w + x] =
                        ((1.0 - 0.09 * (nx * nx + ny * ny).min(2.0)) * scan * 256.0).round() as i32;
                }
            }
            self.dimensions = (w, h);
        }
        let amount = (s.amount * 256.0).round() as i32;
        self.scratch.resize(rgb.len(), 0);
        self.scratch.copy_from_slice(rgb);
        let tick = ((seconds * 24.0).floor() % 4294967296.0) as u32;
        // A subtle fixed chromatic offset: no image-history queue or extra latency.
        let offset = (w / 320).clamp(1, 4);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                let r = self.scratch[(y * w + x.saturating_sub(offset)) * 3] as i32;
                let g = self.scratch[i + 1] as i32;
                let b = self.scratch[(y * w + (x + offset).min(w - 1)) * 3 + 2] as i32;
                let luma = (77 * r + 150 * g + 29 * b) >> 8;
                let grain = noise(x, y, tick);
                let shading = self.shading[y * w + x];
                for (c, value) in [r, g, b].into_iter().enumerate() {
                    let muted = (194 * value + 62 * luma) >> 8;
                    let faded = ((muted * 225) >> 8) + [20, 16, 15][c];
                    let retro = (((faded * shading) >> 8) + grain).clamp(0, 255);
                    let original = self.scratch[i + c] as i32;
                    rgb[i + c] = ((original * (256 - amount) + retro * amount + 128) >> 8) as u8;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn off_and_zero_are_exact_passthrough() {
        let src: Vec<_> = (0..60).collect();
        for s in [
            Settings::default(),
            Settings {
                style: Style::Retro,
                amount: 0.0,
            },
        ] {
            let mut out = src.clone();
            Filter::default().process(&mut out, 5, 4, 1.0, s);
            assert_eq!(src, out);
        }
    }
    #[test]
    fn retro_animates_deterministically_and_reuses_buffer() {
        let src = vec![130; 64 * 48 * 3];
        let s = Settings {
            style: Style::Retro,
            amount: 1.0,
        };
        let mut f = Filter::default();
        let mut a = src.clone();
        f.process(&mut a, 64, 48, 1.0, s);
        let pointer = f.scratch.as_ptr();
        let mut b = src.clone();
        f.process(&mut b, 64, 48, 1.0, s);
        assert_eq!(a, b);
        assert_ne!(a, src);
        assert_eq!(pointer, f.scratch.as_ptr());
        b.copy_from_slice(&src);
        f.process(&mut b, 64, 48, 2.0, s);
        assert_ne!(a, b);
    }
    #[test]
    fn amount_blends_and_tiny_frames_are_safe() {
        let src = vec![120; 8 * 8 * 3];
        let mut full = src.clone();
        let mut half = src.clone();
        let mut f = Filter::default();
        f.process(
            &mut full,
            8,
            8,
            0.0,
            Settings {
                style: Style::Retro,
                amount: 1.0,
            },
        );
        f.process(
            &mut half,
            8,
            8,
            0.0,
            Settings {
                style: Style::Retro,
                amount: 0.5,
            },
        );
        for i in 0..src.len() {
            assert!((half[i] as f32 - (120.0 + full[i] as f32) * 0.5).abs() <= 1.0);
        }
        f.process(
            &mut [42; 3],
            1,
            1,
            1.0,
            Settings {
                style: Style::Retro,
                amount: 1.0,
            },
        );
        assert!(Settings {
            style: Style::Retro,
            amount: f32::NAN
        }
        .validate()
        .is_err());
    }
}
