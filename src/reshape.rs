//! Bounded, local inverse warps. Applied after makeup and before screen overlays.
use crate::landmarks::Landmarks;
#[cfg(test)]
use crate::landmarks::Point;
#[derive(Clone, Copy, Default)]
pub struct Settings {
    pub slim: f32,
    pub chin: f32,
    pub lips: f32,
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.slim.is_finite()
            || !(0.0..=1.0).contains(&self.slim)
            || !self.chin.is_finite()
            || !(-1.0..=1.0).contains(&self.chin)
            || !self.lips.is_finite()
            || !(-1.0..=1.0).contains(&self.lips)
        {
            return Err("Reshape: face-slim 0..1, chin and lip-fullness -1..1".into());
        }
        Ok(())
    }
    pub fn active(self) -> bool {
        self.slim != 0.0 || self.chin != 0.0 || self.lips != 0.0
    }
}
#[derive(Clone, Copy, Default)]
struct Warp {
    center: [f32; 2],
    radius: [f32; 2],
    delta: [f32; 2],
}
#[derive(Default)]
pub struct Reshape {
    scratch: Vec<u8>,
    previous: Option<[[f32; 2]; 5]>,
    dimensions: (usize, usize),
}
fn point(mesh: &Landmarks, i: usize, w: usize, h: usize) -> [f32; 2] {
    [mesh.points[i].x * w as f32, mesh.points[i].y * h as f32]
}
fn inside(x: f32, y: f32, points: &[[f32; 2]]) -> bool {
    let mut result = false;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            result = !result;
        }
    }
    result
}
impl Reshape {
    pub fn process(
        &mut self,
        rgb: &mut [u8],
        w: usize,
        h: usize,
        mesh: Option<&Landmarks>,
        settings: Settings,
    ) {
        if !settings.active() || settings.validate().is_err() {
            self.previous = None;
            return;
        }
        let Some(mesh) = mesh.filter(|m| {
            m.points.len() == 468
                && m.points.iter().all(|p| {
                    p.x.is_finite()
                        && p.y.is_finite()
                        && (-0.5..=1.5).contains(&p.x)
                        && (-0.5..=1.5).contains(&p.y)
                })
        }) else {
            self.previous = None;
            return;
        };
        if w < 2 || h < 2 || rgb.len() != w * h * 3 {
            self.previous = None;
            return;
        }
        let left = point(mesh, 234, w, h);
        let right = point(mesh, 454, w, h);
        let fw = ((right[0] - left[0]).powi(2) + (right[1] - left[1]).powi(2)).sqrt();
        if fw < 20.0 || fw > w as f32 * 1.3 {
            self.previous = None;
            return;
        }
        let axis = [(right[0] - left[0]) / fw, (right[1] - left[1]) / fw];
        let down = [-axis[1], axis[0]];
        let top = point(mesh, 10, w, h);
        let bottom = point(mesh, 152, w, h);
        let fh = (bottom[0] - top[0]) * down[0] + (bottom[1] - top[1]) * down[1];
        if fh < fw * 0.65 || fh > fw * 2.5 {
            self.previous = None;
            return;
        }
        // Reject strong asymmetry: local 2D reshaping is intended for frontal faces.
        let nose = point(mesh, 1, w, h);
        let mid = [(left[0] + right[0]) * 0.5, (left[1] + right[1]) * 0.5];
        if ((nose[0] - mid[0]) * axis[0] + (nose[1] - mid[1]) * axis[1]).abs() > fw * 0.24 {
            self.previous = None;
            return;
        }
        let mut centers = [[0.0; 2]; 5];
        for (j, i) in [172, 397, 152, 0, 17].into_iter().enumerate() {
            centers[j] = point(mesh, i, w, h);
        }
        if self.dimensions == (w, h) {
            if let Some(previous) = self.previous {
                for j in 0..5 {
                    let distance = ((centers[j][0] - previous[j][0]).powi(2)
                        + (centers[j][1] - previous[j][1]).powi(2))
                    .sqrt();
                    if distance < fw * 0.12 {
                        for k in 0..2 {
                            centers[j][k] = previous[j][k] * 0.25 + centers[j][k] * 0.75;
                        }
                    }
                }
            }
        }
        self.previous = Some(centers);
        self.dimensions = (w, h);
        let mouth_left = point(mesh, 61, w, h);
        let mouth_right = point(mesh, 291, w, h);
        let mouth_width = ((mouth_right[0] - mouth_left[0]).powi(2)
            + (mouth_right[1] - mouth_left[1]).powi(2))
        .sqrt();
        let mut warps = [Warp::default(); 5];
        for j in 0..2 {
            warps[j] = Warp {
                center: centers[j],
                radius: [fw * 0.24, fh * 0.23],
                delta: [
                    axis[0] * fw * 0.045 * settings.slim * (if j == 0 { 1.0 } else { -1.0 }),
                    axis[1] * fw * 0.045 * settings.slim * (if j == 0 { 1.0 } else { -1.0 }),
                ],
            };
        }
        warps[2] = Warp {
            center: centers[2],
            radius: [fw * 0.28, fh * 0.20],
            delta: [
                down[0] * fh * 0.035 * settings.chin,
                down[1] * fh * 0.035 * settings.chin,
            ],
        };
        for (j, inner) in [(3, 13), (4, 14)] {
            let p = point(mesh, inner, w, h);
            let thickness =
                ((p[0] - centers[j][0]) * down[0] + (p[1] - centers[j][1]) * down[1]).abs();
            let sign = if j == 3 { -1.0 } else { 1.0 };
            warps[j] = Warp {
                center: centers[j],
                radius: [mouth_width * 0.48, thickness.max(fh * 0.008) * 0.9],
                delta: [
                    down[0] * thickness * 0.35 * settings.lips * sign,
                    down[1] * thickness * 0.35 * settings.lips * sign,
                ],
            };
        }
        let mouth: [[f32; 2]; 20] =
            std::array::from_fn(|i| point(mesh, crate::features::MOUTH[i], w, h));
        self.scratch.resize(rgb.len(), 0);
        self.scratch.copy_from_slice(rgb);
        // Restrict resampling to the union of the small deformation regions.
        let mut min = [w as f32, h as f32];
        let mut max = [0.0f32; 2];
        for warp in &warps {
            if warp.delta == [0.0; 2] {
                continue;
            }
            let radius = warp.radius[0].max(warp.radius[1]);
            for k in 0..2 {
                min[k] = min[k].min(warp.center[k] + warp.delta[k] - radius);
                max[k] = max[k].max(warp.center[k] + warp.delta[k] + radius);
            }
        }
        for y in min[1].max(0.0) as usize..(max[1].ceil() as usize).min(h) {
            for x in min[0].max(0.0) as usize..(max[0].ceil() as usize).min(w) {
                let mut delta = [0.0; 2];
                let mut lip_delta = [0.0; 2];
                for (j, warp) in warps.iter().enumerate() {
                    if warp.delta == [0.0; 2] {
                        continue;
                    }
                    let dx = x as f32 - warp.center[0] - warp.delta[0];
                    let dy = y as f32 - warp.center[1] - warp.delta[1];
                    let u = (dx * axis[0] + dy * axis[1]) / warp.radius[0];
                    let v = (dx * down[0] + dy * down[1]) / warp.radius[1];
                    let r = u * u + v * v;
                    if r < 1.0 {
                        let weight = (1.0 - r).powi(2);
                        let d = if j >= 3 { &mut lip_delta } else { &mut delta };
                        for k in 0..2 {
                            d[k] += warp.delta[k] * weight;
                        }
                    }
                }
                if lip_delta != [0.0; 2]
                    && !inside(x as f32, y as f32, &mouth)
                    && !inside(
                        x as f32 - delta[0] - lip_delta[0],
                        y as f32 - delta[1] - lip_delta[1],
                        &mouth,
                    )
                {
                    for k in 0..2 {
                        delta[k] += lip_delta[k];
                    }
                }
                if delta == [0.0; 2] {
                    continue;
                }
                let sx = (x as f32 - delta[0]).clamp(0.0, (w - 1) as f32);
                let sy = (y as f32 - delta[1]).clamp(0.0, (h - 1) as f32);
                let ix = sx as usize;
                let iy = sy as usize;
                let fx = sx - ix as f32;
                let fy = sy - iy as f32;
                for c in 0..3 {
                    let a = self.scratch[(iy * w + ix) * 3 + c] as f32;
                    let b = self.scratch[(iy * w + (ix + 1).min(w - 1)) * 3 + c] as f32;
                    let d = self.scratch[(((iy + 1).min(h - 1)) * w + ix) * 3 + c] as f32;
                    let e = self.scratch[(((iy + 1).min(h - 1)) * w + (ix + 1).min(w - 1)) * 3 + c]
                        as f32;
                    rgb[(y * w + x) * 3 + c] =
                        ((a + (b - a) * fx) * (1.0 - fy) + (d + (e - d) * fx) * fy).round() as u8;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mesh() -> Landmarks {
        let mut m = Landmarks {
            points: vec![Point { x: 0.5, y: 0.5 }; 468],
        };
        for (i, x, y) in [
            (234, 0.2, 0.45),
            (454, 0.8, 0.45),
            (10, 0.5, 0.1),
            (152, 0.5, 0.9),
            (172, 0.25, 0.7),
            (397, 0.75, 0.7),
            (61, 0.35, 0.65),
            (291, 0.65, 0.65),
            (0, 0.5, 0.60),
            (17, 0.5, 0.70),
            (13, 0.5, 0.64),
            (14, 0.5, 0.66),
        ] {
            m.points[i] = Point { x, y };
        }
        for (j, &i) in crate::features::MOUTH.iter().enumerate() {
            let a = j as f32 / 20.0 * std::f32::consts::TAU;
            m.points[i] = Point {
                x: 0.5 + a.cos() * 0.13,
                y: 0.65 + a.sin() * 0.01,
            };
        }
        m.points[13] = Point { x: 0.5, y: 0.64 };
        m.points[14] = Point { x: 0.5, y: 0.66 };
        m
    }
    #[test]
    fn neutral_missing_and_invalid_mesh_are_exact_passthrough() {
        let src = vec![100; 100 * 100 * 3];
        let mut out = src.clone();
        let mut r = Reshape::default();
        r.process(&mut out, 100, 100, Some(&mesh()), Settings::default());
        assert_eq!(out, src);
        let s = Settings {
            slim: 1.0,
            ..Settings::default()
        };
        r.process(&mut out, 100, 100, None, s);
        assert_eq!(out, src);
        let mut bad = mesh();
        bad.points[1].x = f32::NAN;
        r.process(&mut out, 100, 100, Some(&bad), s);
        assert_eq!(out, src);
        assert!(r.previous.is_none());
    }
    #[test]
    fn moves_local_regions_and_preserves_eyes_and_background() {
        let mut src = vec![0; 200 * 200 * 3];
        for y in 0..200 {
            for x in 0..200 {
                src[(y * 200 + x) * 3] = x as u8;
                src[(y * 200 + x) * 3 + 1] = y as u8;
            }
        }
        let mut r = Reshape::default();
        for s in [
            Settings {
                slim: 1.0,
                ..Settings::default()
            },
            Settings {
                chin: 1.0,
                ..Settings::default()
            },
            Settings {
                lips: 1.0,
                ..Settings::default()
            },
        ] {
            let mut out = src.clone();
            r.process(&mut out, 200, 200, Some(&mesh()), s);
            assert_ne!(src, out);
            assert_eq!(&out[..200 * 60 * 3], &src[..200 * 60 * 3]);
            assert_eq!(&out[..30], &src[..30]);
        }
    }
    #[test]
    fn signed_warps_move_in_expected_directions_and_protect_mouth() {
        let (w, h) = (200, 200);
        let mut src = vec![0; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                src[(y * w + x) * 3] = x as u8;
                src[(y * w + x) * 3 + 1] = y as u8;
            }
        }
        let m = mesh();
        for sign in [-1.0, 1.0] {
            let mut r = Reshape::default();
            let mut out = src.clone();
            r.process(
                &mut out,
                w,
                h,
                Some(&m),
                Settings {
                    chin: sign,
                    ..Settings::default()
                },
            );
            let i = (180 * w + 100) * 3 + 1;
            assert!((out[i] as f32 - src[i] as f32) * sign < 0.0);
            out.copy_from_slice(&src);
            r.process(
                &mut out,
                w,
                h,
                Some(&m),
                Settings {
                    lips: sign,
                    ..Settings::default()
                },
            );
            let i = (120 * w + 100) * 3 + 1;
            assert!((out[i] as f32 - src[i] as f32) * sign > 0.0);
            for y in 129..132 {
                for x in 90..111 {
                    let i = (y * w + x) * 3;
                    assert_eq!(&out[i..i + 3], &src[i..i + 3]);
                }
            }
        }
        let mut out = src.clone();
        Reshape::default().process(
            &mut out,
            w,
            h,
            Some(&m),
            Settings {
                slim: 1.0,
                ..Settings::default()
            },
        );
        assert!(out[(140 * w + 52) * 3] < src[(140 * w + 52) * 3]);
        assert!(out[(140 * w + 148) * 3] > src[(140 * w + 148) * 3]);
    }
    #[test]
    fn parameter_ranges_are_bounded() {
        for s in [
            Settings {
                slim: f32::NAN,
                ..Settings::default()
            },
            Settings {
                chin: 1.1,
                ..Settings::default()
            },
            Settings {
                lips: -1.1,
                ..Settings::default()
            },
        ] {
            assert!(s.validate().is_err());
        }
    }
}
