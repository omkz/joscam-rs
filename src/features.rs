//! Rasterize canonical MediaPipe contours into cached per-pixel masks.
use crate::landmarks::{Landmarks, Point};
pub const FACE: &[usize] = &[
    10, 338, 297, 332, 284, 251, 389, 356, 454, 323, 361, 288, 397, 365, 379, 378, 400, 377, 152,
    148, 176, 149, 150, 136, 172, 58, 132, 93, 234, 127, 162, 21, 54, 103, 67, 109,
];
pub const LIPS: &[usize] = &[
    61, 146, 91, 181, 84, 17, 314, 405, 321, 375, 291, 409, 270, 269, 267, 0, 37, 39, 40, 185,
];
pub const MOUTH: &[usize] = &[
    78, 95, 88, 178, 87, 14, 317, 402, 318, 324, 308, 415, 310, 311, 312, 13, 82, 81, 80, 191,
];
pub const BROW_LEFT: &[usize] = &[70, 63, 105, 66, 107, 55, 65, 52, 53, 46];
pub const BROW_RIGHT: &[usize] = &[336, 296, 334, 293, 300, 276, 283, 282, 295, 285];
pub const EYE_LEFT: &[usize] = &[
    33, 7, 163, 144, 145, 153, 154, 155, 133, 173, 157, 158, 159, 160, 161, 246,
];
pub const EYE_RIGHT: &[usize] = &[
    263, 249, 390, 373, 374, 380, 381, 382, 362, 398, 384, 385, 386, 387, 388, 466,
];

pub fn polygon(mask: &mut [f32], points: &[Point], width: usize, height: usize, value: f32) {
    if points.len() < 3 {
        return;
    }
    let min_y = points
        .iter()
        .map(|p| p.y * height as f32)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_y = points
        .iter()
        .map(|p| p.y * height as f32)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(height as f32)
        .max(0.0) as usize;
    let mut cuts = Vec::with_capacity(points.len());
    for y in min_y..max_y {
        cuts.clear();
        let fy = (y as f32 + 0.5) / height as f32;
        for k in 0..points.len() {
            let a = points[k];
            let b = points[(k + 1) % points.len()];
            if (a.y > fy) != (b.y > fy) {
                cuts.push((a.x + (fy - a.y) * (b.x - a.x) / (b.y - a.y)) * width as f32);
            }
        }
        cuts.sort_by(f32::total_cmp);
        for pair in cuts.chunks_exact(2) {
            let x0 = (pair[0] - 0.5).ceil().max(0.0).min(width as f32) as usize;
            let x1 = (pair[1] - 0.5).ceil().max(0.0).min(width as f32) as usize;
            for x in x0..x1 {
                mask[y * width + x] = value;
            }
        }
    }
}
fn contour(mesh: &Landmarks, indices: &[usize]) -> Vec<Point> {
    indices.iter().map(|&i| mesh.points[i]).collect()
}
pub fn masks(
    mesh: &Landmarks,
    width: usize,
    height: usize,
    skin: &mut [f32],
    lips: &mut [f32],
    eyes: &mut [f32],
) {
    skin.fill(0.0);
    lips.fill(0.0);
    eyes.fill(0.0);
    if mesh.points.len() != 468 {
        return;
    }
    polygon(skin, &contour(mesh, FACE), width, height, 1.0);
    let face_width = (mesh.points[454].x - mesh.points[234].x).abs() * width as f32;
    feather_inward(
        skin,
        width,
        height,
        (face_width * 0.10).clamp(4.0, 40.0) as u16,
    );

    polygon(lips, &contour(mesh, LIPS), width, height, 1.0);
    polygon(lips, &contour(mesh, MOUTH), width, height, 0.0);
    polygon(skin, &contour(mesh, LIPS), width, height, 0.0);
    for eye in [EYE_LEFT, EYE_RIGHT] {
        let p = contour(mesh, eye);
        polygon(skin, &p, width, height, 0.0);
        polygon(eyes, &p, width, height, 1.0);
    }
    for brow in [BROW_LEFT, BROW_RIGHT] {
        polygon(skin, &contour(mesh, brow), width, height, 0.0);
    }
    // Feather inside contours only, so teeth and background remain unchanged.
    feather_inward(skin, width, height, 3);
    feather_inward(lips, width, height, 1);
    feather_inward(eyes, width, height, 1);
}

fn feather_inward(mask: &mut [f32], width: usize, height: usize, radius: u16) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    let mut distance: Vec<u16> = mask
        .iter()
        .map(|&v| if v > 0.0 { radius } else { 0 })
        .collect();
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if x > 0 {
                distance[i] = distance[i].min(distance[i - 1] + 1);
            }
            if y > 0 {
                distance[i] = distance[i].min(distance[i - width] + 1);
            }
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let i = y * width + x;
            if x + 1 < width {
                distance[i] = distance[i].min(distance[i + 1] + 1);
            }
            if y + 1 < height {
                distance[i] = distance[i].min(distance[i + width] + 1);
            }
            let t = distance[i] as f32 / radius as f32;
            mask[i] *= t * t * (3.0 - 2.0 * t);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn mesh() -> Landmarks {
        let mut mesh = Landmarks {
            points: vec![Point { x: 0.5, y: 0.5 }; 468],
        };
        for (indices, cx, cy, rx, ry) in [
            (FACE, 0.5, 0.5, 0.35, 0.45),
            (LIPS, 0.5, 0.72, 0.14, 0.06),
            (MOUTH, 0.5, 0.72, 0.09, 0.02),
            (EYE_LEFT, 0.35, 0.38, 0.08, 0.035),
            (EYE_RIGHT, 0.65, 0.38, 0.08, 0.035),
        ] {
            for (k, &index) in indices.iter().enumerate() {
                let a = k as f32 / indices.len() as f32 * std::f32::consts::TAU;
                mesh.points[index] = Point {
                    x: cx + rx * a.cos(),
                    y: cy + ry * a.sin(),
                };
            }
        }
        mesh
    }
    #[test]
    fn masks_protect_eye_mouth_and_background() {
        let (mut skin, mut lip, mut eyes) = (vec![0.0; 10000], vec![0.0; 10000], vec![0.0; 10000]);
        masks(&mesh(), 100, 100, &mut skin, &mut lip, &mut eyes);
        assert_eq!(skin[0], 0.0);
        assert_eq!(lip[72 * 100 + 50], 0.0);
        assert_eq!(skin[38 * 100 + 35], 0.0);
        assert!(eyes[38 * 100 + 35] > 0.0);
        assert!(lip[76 * 100 + 50] > 0.0);
        assert!(skin[55 * 100 + 50] > 0.0);
    }
    #[test]
    fn offscreen_polygon_is_clipped() {
        let mut mask = vec![0.0; 100];
        polygon(
            &mut mask,
            &[
                Point { x: -1.0, y: -1.0 },
                Point { x: 0.5, y: -1.0 },
                Point { x: 0.5, y: 0.5 },
                Point { x: -1.0, y: 0.5 },
            ],
            10,
            10,
            1.0,
        );
        assert_eq!(mask.iter().sum::<f32>(), 25.0);
    }
}
