/// Lightweight retouching inside a centered portrait ellipse. This is a
/// color heuristic, not face detection or generative de-aging.
pub fn process(src: &[u8], dst: &mut [u8], width: usize, height: usize, amount: f32, light: f32) {
    assert_eq!(src.len(), width * height * 3);
    assert_eq!(dst.len(), src.len());
    dst.copy_from_slice(src);
    let amount = amount.clamp(0.0, 1.0);
    let light = light.clamp(0.0, 1.0);
    if amount == 0.0 && light == 0.0 {
        return;
    }
    for y in 2..height.saturating_sub(2) {
        for x in 2..width.saturating_sub(2) {
            let nx = (x as f32 / width as f32 - 0.5) / 0.24;
            let ny = (y as f32 / height as f32 - 0.46) / 0.43;
            let spatial = ((1.0 - nx * nx - ny * ny) * 5.0).clamp(0.0, 1.0);
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
            let mut sum = [0.0f32; 3];
            let mut weights = 0.0;
            for dy in -2isize..=2 {
                for dx in -2isize..=2 {
                    let j = (((y as isize + dy) as usize) * width + (x as isize + dx) as usize) * 3;
                    let delta = (0..3)
                        .map(|c| (src[j + c] as f32 - src[i + c] as f32).abs())
                        .sum::<f32>();
                    let w = 1.0 / (1.0 + delta * delta / 400.0 + (dx * dx + dy * dy) as f32 / 4.0);
                    weights += w;
                    for c in 0..3 {
                        sum[c] += src[j + c] as f32 * w;
                    }
                }
            }
            for c in 0..3 {
                let original = src[i + c] as f32;
                let smooth = sum[c] / weights;
                let target = original + (smooth - original) * amount * 0.8;
                let target = target + light * 0.2 * target * (1.0 - target / 255.0);
                dst[i + c] = (original + (target - original) * mask)
                    .clamp(0.0, 255.0)
                    .round() as u8;
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
        process(&src, &mut dst, 20, 20, 0.0, 0.0);
        assert_eq!(src, dst);
    }
    #[test]
    fn blue_background_is_preserved() {
        let src = [20, 40, 200].repeat(400);
        let mut dst = src.clone();
        process(&src, &mut dst, 20, 20, 1.0, 1.0);
        assert_eq!(src, dst);
    }
    #[test]
    fn skin_midtones_lift_only_in_portrait_region() {
        let src = [170, 120, 100].repeat(400);
        let mut dst = src.clone();
        process(&src, &mut dst, 20, 20, 0.0, 1.0);
        assert!(dst[(10 * 20 + 10) * 3] > src[(10 * 20 + 10) * 3]);
        assert_eq!(&dst[..3], &src[..3]);
    }
    #[test]
    fn tiny_frames_do_not_panic() {
        for w in 1..5 {
            for h in 1..5 {
                let src = vec![100; w * h * 3];
                let mut dst = src.clone();
                process(&src, &mut dst, w, h, 1.0, 1.0);
            }
        }
    }
}
