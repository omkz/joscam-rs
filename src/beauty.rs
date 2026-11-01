/// Local cosmetic retouching. The mask is a centered portrait/color heuristic,
/// not face tracking. Scratch storage and the spatial mask are reused.
#[derive(Default)]
pub struct Beauty {
    /// Cosmetic lift toward white inside the existing skin mask, 0..1.
    pub skin_brightening: f32,
    shape: (usize, usize),
    horizontal: Vec<f32>,
    blurred: Vec<f32>,
    column_sums: Vec<f32>,
    spatial: Vec<f32>,
}

impl Beauty {
    fn prepare(&mut self, width: usize, height: usize) {
        if self.shape == (width, height) {
            return;
        }
        self.shape = (width, height);
        self.horizontal.resize(width * height * 3, 0.0);
        self.blurred.resize(width * height * 3, 0.0);
        self.column_sums.resize(width * 3, 0.0);
        self.spatial.resize(width * height, 0.0);
        for y in 0..height {
            for x in 0..width {
                let nx = (x as f32 / width as f32 - 0.5) / 0.24;
                let ny = (y as f32 / height as f32 - 0.46) / 0.43;
                self.spatial[y * width + x] = ((1.0 - nx * nx - ny * ny) * 5.0).clamp(0.0, 1.0);
            }
        }
    }

    // Separable running sums: constant work per pixel instead of 25 RGB
    // range-weight evaluations. Clamp edges rather than introducing dark borders.
    fn blur(&mut self, src: &[u8], width: usize, height: usize) {
        for y in 0..height {
            for c in 0..3 {
                let at = |x: usize| src[(y * width + x) * 3 + c] as f32;
                let mut sum = at(0) * 3.0 + at(1.min(width - 1)) + at(2.min(width - 1));
                for x in 0..width {
                    self.horizontal[(y * width + x) * 3 + c] = sum / 5.0;
                    sum += at((x + 3).min(width - 1)) - at(x.saturating_sub(2));
                }
            }
        }
        let stride = width * 3;
        for i in 0..stride {
            self.column_sums[i] = self.horizontal[i] * 3.0
                + self.horizontal[1.min(height - 1) * stride + i]
                + self.horizontal[2.min(height - 1) * stride + i];
        }
        for y in 0..height {
            let add = (y + 3).min(height - 1) * stride;
            let remove = y.saturating_sub(2) * stride;
            for i in 0..stride {
                self.blurred[y * stride + i] = self.column_sums[i] / 5.0;
                self.column_sums[i] += self.horizontal[add + i] - self.horizontal[remove + i];
            }
        }
    }

    pub fn process(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
        amount: f32,
        light: f32,
    ) {
        assert_eq!(src.len(), width * height * 3);
        assert_eq!(dst.len(), src.len());
        dst.copy_from_slice(src);
        let amount = amount.clamp(0.0, 1.0);
        let light = light.clamp(0.0, 1.0);
        let brightening = self.skin_brightening.clamp(0.0, 1.0);
        if width == 0 || height == 0 || (amount == 0.0 && light == 0.0 && brightening == 0.0) {
            return;
        }
        self.prepare(width, height);
        if amount > 0.0 {
            self.blur(src, width, height);
        }
        for y in 2..height.saturating_sub(2) {
            for x in 2..width.saturating_sub(2) {
                let spatial = self.spatial[y * width + x];
                if spatial == 0.0 {
                    continue;
                }
                let i = (y * width + x) * 3;
                let r = src[i] as f32;
                let g = src[i + 1] as f32;
                let b = src[i + 2] as f32;
                let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
                let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;
                let skin = ((cb - 75.0) / 12.0).clamp(0.0, 1.0)
                    * ((135.0 - cb) / 12.0).clamp(0.0, 1.0)
                    * ((cr - 130.0) / 12.0).clamp(0.0, 1.0)
                    * ((185.0 - cr) / 12.0).clamp(0.0, 1.0);
                let mask = skin * spatial;
                if mask == 0.0 {
                    continue;
                }
                // Reduce smoothing near strong color/detail transitions. This is
                // an approximation, not numerically identical to bilateral blur.
                let edge = if amount > 0.0 {
                    let delta = (self.blurred[i] - r).abs()
                        + (self.blurred[i + 1] - g).abs()
                        + (self.blurred[i + 2] - b).abs();
                    (1.0 - delta / 45.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                for c in 0..3 {
                    let original = src[i + c] as f32;
                    let target = if amount > 0.0 {
                        original + (self.blurred[i + c] - original) * amount * 0.8 * edge
                    } else {
                        original
                    };
                    let target = target + light * 0.2 * target * (1.0 - target / 255.0);
                    // A bounded blend toward white lifts and gently desaturates
                    // skin, preserving channel ordering and highlight headroom.
                    let target = target + (255.0 - target) * brightening * 0.35;
                    dst[i + c] = (original + (target - original) * mask)
                        .clamp(0.0, 255.0)
                        .round() as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_is_exact_passthrough() {
        let src: Vec<u8> = (0..1200).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0; src.len()];
        Beauty::default().process(&src, &mut dst, 20, 20, 0.0, 0.0);
        assert_eq!(src, dst);
    }
    #[test]
    fn blue_background_is_preserved() {
        let src = [20, 40, 200].repeat(400);
        let mut dst = src.clone();
        Beauty::default().process(&src, &mut dst, 20, 20, 1.0, 1.0);
        assert_eq!(src, dst);
    }
    #[test]
    fn skin_midtones_lift_only_in_portrait_region() {
        let src = [170, 120, 100].repeat(400);
        let mut dst = src.clone();
        Beauty::default().process(&src, &mut dst, 20, 20, 0.0, 1.0);
        assert!(dst[(10 * 20 + 10) * 3] > src[(10 * 20 + 10) * 3]);
        assert_eq!(&dst[..3], &src[..3]);
    }
    #[test]
    fn tiny_frames_do_not_panic() {
        for w in 1..5 {
            for h in 1..5 {
                let src = vec![100; w * h * 3];
                let mut dst = src.clone();
                Beauty::default().process(&src, &mut dst, w, h, 1.0, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn blur_keeps_uniform_frame_uniform_including_edges() {
        let mut filter = Beauty::default();
        filter.prepare(8, 6);
        filter.blur(&[123; 8 * 6 * 3], 8, 6);
        assert!(filter.blurred.iter().all(|v| (*v - 123.0).abs() < 0.001));
    }
    #[test]
    fn smoother_reduces_skin_texture_and_reuses_buffers() {
        let (w, h) = (64, 48);
        let src: Vec<u8> = (0..w * h * 3)
            .map(|i| ([170i32, 120, 100][i % 3] + if (i / 3) % 2 == 0 { 5 } else { -5 }) as u8)
            .collect();
        let mut dst = src.clone();
        let mut filter = Beauty::default();
        filter.process(&src, &mut dst, w, h, 1.0, 0.0);
        let variation = |frame: &[u8]| {
            (20..40)
                .map(|x| {
                    (frame[(24 * w + x) * 3] as i32 - frame[(24 * w + x + 1) * 3] as i32).abs()
                })
                .sum::<i32>()
        };
        assert!(variation(&dst) < variation(&src));
        let ptr = filter.horizontal.as_ptr();
        filter.process(&src, &mut dst, w, h, 0.6, 0.2);
        assert_eq!(ptr, filter.horizontal.as_ptr());
        filter.process(&[100; 8 * 8 * 3], &mut [0; 8 * 8 * 3], 8, 8, 0.6, 0.2);
        assert_eq!(filter.shape, (8, 8));
    }
    #[test]
    #[ignore = "manual release performance measurement"]
    fn benchmark_beauty() {
        for (w, h) in [(640, 480), (1280, 720)] {
            let src: Vec<u8> = (0..w * h * 3)
                .map(|i| ([170i32, 120, 100][i % 3] + (i * 13 % 23) as i32 - 11) as u8)
                .collect();
            let mut dst = src.clone();
            let mut filter = Beauty::default();
            filter.process(&src, &mut dst, w, h, 0.65, 0.3);
            let start = std::time::Instant::now();
            for _ in 0..60 {
                filter.process(
                    std::hint::black_box(&src),
                    std::hint::black_box(&mut dst),
                    w,
                    h,
                    0.65,
                    0.3,
                );
            }
            eprintln!(
                "{w}x{h}: {:.2} ms/frame",
                start.elapsed().as_secs_f64() * 1000.0 / 60.0
            );
        }
    }
}

#[cfg(test)]
mod brightening_tests {
    use super::*;
    #[test]
    fn brightening_is_local_monotonic_and_preserves_channel_order() {
        let src = [170, 120, 100].repeat(64 * 48);
        let mut low = src.clone();
        let mut high = src.clone();
        let mut filter = Beauty::default();
        filter.skin_brightening = 0.3;
        filter.process(&src, &mut low, 64, 48, 0.0, 0.0);
        filter.skin_brightening = 0.8;
        filter.process(&src, &mut high, 64, 48, 0.0, 0.0);
        let i = (24 * 64 + 32) * 3;
        for c in 0..3 {
            assert!(high[i + c] > low[i + c]);
            assert!(low[i + c] > src[i + c]);
        }
        assert!(high[i] > high[i + 1] && high[i + 1] > high[i + 2]);
        assert_eq!(&high[..3], &src[..3]);
    }
    #[test]
    fn brightening_keeps_non_skin_and_white_unchanged() {
        let mut filter = Beauty::default();
        filter.skin_brightening = 1.0;
        for color in [[20, 40, 200], [255, 255, 255], [0, 0, 0]] {
            let src = color.repeat(64 * 48);
            let mut dst = src.clone();
            filter.process(&src, &mut dst, 64, 48, 0.0, 0.0);
            assert_eq!(src, dst);
        }
    }
}
