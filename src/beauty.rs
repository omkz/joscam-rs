/// Local cosmetic retouching. The skin-color mask is either centered or
/// restricted to a detected face box. Scratch storage is reused.
#[derive(Default)]
pub struct Beauty {
    /// Cosmetic lift toward white inside the existing skin mask, 0..1.
    pub skin_brightening: f32,
    pub rosy: f32,
    pub glow: f32,
    pub lip_tint: f32,
    pub eye_brightening: f32,
    landmarks: Option<crate::landmarks::Landmarks>,
    lip_mask: Vec<f32>,
    eye_mask: Vec<f32>,
    tracking: bool,
    face: Option<crate::face::FaceRect>,
    spatial_dirty: bool,
    shape: (usize, usize),
    horizontal: Vec<f32>,
    blurred: Vec<f32>,
    column_sums: Vec<f32>,
    spatial: Vec<f32>,
}

impl Beauty {
    pub fn set_landmarks(&mut self, landmarks: Option<crate::landmarks::Landmarks>) {
        let landmarks = landmarks.filter(|m| m.points.len() == 468);
        if self.landmarks != landmarks {
            self.spatial_dirty = true;
        }
        self.landmarks = landmarks;
    }

    pub fn set_tracked_face(&mut self, face: Option<crate::face::FaceRect>) {
        if !self.tracking || self.face != face {
            self.spatial_dirty = true;
        }
        self.tracking = true;
        self.face = face;
    }

    fn prepare(&mut self, width: usize, height: usize) {
        if self.shape == (width, height) && !self.spatial_dirty {
            return;
        }
        self.shape = (width, height);
        self.spatial_dirty = false;
        self.horizontal.resize(width * height * 3, 0.0);
        self.blurred.resize(width * height * 3, 0.0);
        self.column_sums.resize(width * 3, 0.0);
        self.spatial.resize(width * height, 0.0);
        self.lip_mask.resize(width * height, 0.0);
        self.lip_mask.fill(0.0);
        self.eye_mask.resize(width * height, 0.0);
        self.eye_mask.fill(0.0);
        for y in 0..height {
            for x in 0..width {
                let px = x as f32 / width as f32;
                let py = y as f32 / height as f32;
                let (nx, ny) = if self.tracking {
                    let Some(f) = self.face else {
                        self.spatial[y * width + x] = 0.0;
                        continue;
                    };
                    (
                        (px - f.x - f.w * 0.5) / (f.w * 0.46).max(0.001),
                        (py - f.y - f.h * 0.52) / (f.h * 0.50).max(0.001),
                    )
                } else {
                    ((px - 0.5) / 0.24, (py - 0.46) / 0.43)
                };
                let mut mask = ((1.0 - nx * nx - ny * ny) * 5.0).clamp(0.0, 1.0);
                if self.tracking {
                    // Broad box-relative feature protection; these are NOT landmarks.
                    for (cx, cy, rx, ry) in [
                        (-0.42, -0.27, 0.46, 0.25),
                        (0.42, -0.27, 0.46, 0.25),
                        (0.0, 0.51, 0.55, 0.22),
                    ] {
                        let d = ((nx - cx) / rx).powi(2) + ((ny - cy) / ry).powi(2);
                        mask *= ((d - 0.7) * 3.0).clamp(0.0, 1.0);
                    }
                }
                self.spatial[y * width + x] = mask;
            }
        }
        if let Some(mesh) = &self.landmarks {
            crate::features::masks(
                mesh,
                width,
                height,
                &mut self.spatial,
                &mut self.lip_mask,
                &mut self.eye_mask,
            );
        }
    }

    // Separable running sums: constant work per pixel instead of 25 RGB
    // range-weight evaluations. Clamp edges rather than introducing dark borders.
    fn blur(&mut self, src: &[u8], width: usize, height: usize) {
        let radius = if let Some(face) = self.face {
            (face.w * width as f32 * 0.025).round().clamp(2.0, 6.0) as usize
        } else {
            (height.min(width) / 150).clamp(2, 6)
        };
        let divisor = (2 * radius + 1) as f32;
        for y in 0..height {
            for c in 0..3 {
                let at = |x: usize| src[(y * width + x) * 3 + c] as f32;
                let mut sum = at(0) * (radius + 1) as f32;
                for step in 1..=radius {
                    sum += at(step.min(width - 1));
                }
                for x in 0..width {
                    self.horizontal[(y * width + x) * 3 + c] = sum / divisor;
                    sum += at((x + radius + 1).min(width - 1)) - at(x.saturating_sub(radius));
                }
            }
        }
        let stride = width * 3;
        for i in 0..stride {
            self.column_sums[i] = self.horizontal[i] * (radius + 1) as f32;
            for step in 1..=radius {
                self.column_sums[i] += self.horizontal[step.min(height - 1) * stride + i];
            }
        }
        for y in 0..height {
            let add = (y + radius + 1).min(height - 1) * stride;
            let remove = y.saturating_sub(radius) * stride;
            for i in 0..stride {
                self.blurred[y * stride + i] = self.column_sums[i] / divisor;
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
        let rosy = self.rosy.clamp(0.0, 1.0);
        let glow = self.glow.clamp(0.0, 1.0);
        if width == 0
            || height == 0
            || (amount == 0.0
                && light == 0.0
                && brightening == 0.0
                && rosy == 0.0
                && glow == 0.0
                && self.lip_tint == 0.0
                && self.eye_brightening == 0.0)
        {
            return;
        }
        self.prepare(width, height);
        if self.tracking && self.face.is_none() {
            return;
        }
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
                // Dense geometry allows a broader skin mask. Protect dark hair
                // and highly saturated pixels rather than dropping neutral skin.
                let skin = if self.landmarks.is_some() {
                    skin.max(0.65)
                        * ((r.min(g).min(b) - 25.0) / 45.0).clamp(0.0, 1.0)
                        * (1.0 - (r.max(g).max(b) - r.min(g).min(b) - 100.0).max(0.0) / 80.0)
                            .clamp(0.0, 1.0)
                } else {
                    skin
                };
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
                    (1.0 - delta / 90.0).clamp(0.0, 1.0)
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
                    let target = target + light * 0.45 * target * (1.0 - target / 255.0);
                    // A bounded blend toward white lifts and gently desaturates
                    // skin, preserving channel ordering and highlight headroom.
                    let target = target + (255.0 - target) * brightening * 0.50;
                    let target = target + glow * 0.12 * target * (1.0 - target / 255.0);
                    let tint = [1.0, 0.10, 0.40][c];
                    let target = target + (255.0 - target) * rosy * 0.08 * tint;
                    dst[i + c] = (original + (target - original) * mask)
                        .clamp(0.0, 255.0)
                        .round() as u8;
                }
            }
        }
        if self.landmarks.is_some() {
            self.apply_makeup(src, dst);
        }
    }
    fn apply_makeup(&self, src: &[u8], dst: &mut [u8]) {
        for (pixel, (original, out)) in src.chunks_exact(3).zip(dst.chunks_exact_mut(3)).enumerate()
        {
            let lip = self.lip_mask[pixel] * self.lip_tint.clamp(0.0, 1.0);
            if lip > 0.0 {
                let luminance = (0.299 * original[0] as f32
                    + 0.587 * original[1] as f32
                    + 0.114 * original[2] as f32)
                    / 255.0;
                let shade = 0.45 + luminance * 0.70;
                for c in 0..3 {
                    let tint = [210.0, 65.0, 100.0][c] * shade;
                    out[c] = (out[c] as f32 * (1.0 - lip * 0.65) + tint * lip * 0.65)
                        .clamp(0.0, 255.0)
                        .round() as u8;
                }
            }
            let eye = self.eye_mask[pixel] * self.eye_brightening.clamp(0.0, 1.0);
            if eye > 0.0 {
                let min = *original.iter().min().unwrap() as f32;
                let max = *original.iter().max().unwrap() as f32;
                let whites = ((min - 70.0) / 120.0).clamp(0.0, 1.0)
                    * (1.0 - (max - min) / 100.0).clamp(0.0, 1.0);
                for c in 0..3 {
                    out[c] = (out[c] as f32 + (255.0 - out[c] as f32) * eye * whites * 0.30).round()
                        as u8;
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

#[cfg(test)]
mod tracking_tests {
    use super::*;
    use crate::face::FaceRect;
    #[test]
    fn tracking_without_face_is_exact_passthrough() {
        let src = [170, 120, 100].repeat(80 * 60);
        let mut dst = src.clone();
        let mut b = Beauty::default();
        b.skin_brightening = 0.5;
        b.rosy = 0.5;
        b.glow = 0.5;
        b.set_tracked_face(None);
        b.process(&src, &mut dst, 80, 60, 0.7, 0.4);
        assert_eq!(src, dst);
    }
    #[test]
    fn tracked_mask_follows_face_and_clears_on_loss() {
        let src = [170, 120, 100].repeat(80 * 60);
        let mut dst = src.clone();
        let mut b = Beauty::default();
        b.skin_brightening = 0.5;
        b.set_tracked_face(Some(FaceRect {
            x: 0.1,
            y: 0.1,
            w: 0.3,
            h: 0.7,
        }));
        b.process(&src, &mut dst, 80, 60, 0.0, 0.0);
        let left = (30 * 80 + 20) * 3;
        let right = (30 * 80 + 60) * 3;
        assert!(dst[left] > src[left]);
        assert_eq!(dst[right], src[right]);
        b.set_tracked_face(Some(FaceRect {
            x: 0.6,
            y: 0.1,
            w: 0.3,
            h: 0.7,
        }));
        b.process(&src, &mut dst, 80, 60, 0.0, 0.0);
        assert_eq!(dst[left], src[left]);
        assert!(dst[right] > src[right]);
        b.set_tracked_face(None);
        b.process(&src, &mut dst, 80, 60, 0.0, 0.0);
        assert_eq!(src, dst);
    }
}

#[cfg(test)]
mod makeup_tests {
    use super::*;
    #[test]
    fn lipstick_preserves_mouth_cavity_and_eye_effect_preserves_dark_pupils() {
        let src = [130, 110, 105].repeat(10000);
        let mut dst = src.clone();
        let mut beauty = Beauty::default();
        beauty.lip_tint = 1.0;
        beauty.set_landmarks(Some(crate::features::tests::mesh()));
        beauty.process(&src, &mut dst, 100, 100, 0.0, 0.0);
        let lip = (76 * 100 + 50) * 3;
        let mouth = (72 * 100 + 50) * 3;
        assert_ne!(&dst[lip..lip + 3], &src[lip..lip + 3]);
        assert_eq!(&dst[mouth..mouth + 3], &src[mouth..mouth + 3]);
        beauty.lip_tint = 0.0;
        beauty.eye_brightening = 1.0;
        let src = [170, 170, 170].repeat(10000);
        let mut dst = src.clone();
        beauty.process(&src, &mut dst, 100, 100, 0.0, 0.0);
        let eye = (38 * 100 + 35) * 3;
        assert!(dst[eye] > src[eye]);
        let src = [20, 20, 20].repeat(10000);
        let mut dst = src.clone();
        beauty.process(&src, &mut dst, 100, 100, 0.0, 0.0);
        assert_eq!(src, dst);
    }
}
