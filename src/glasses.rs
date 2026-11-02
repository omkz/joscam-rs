//! Lightweight procedural eyeglass frames anchored to detected eye landmarks.
use crate::landmarks::{Landmarks, Point};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    Off,
    Round,
    Square,
    Sunglasses,
}
impl Style {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "off" => Ok(Self::Off),
            "round" => Ok(Self::Round),
            "square" => Ok(Self::Square),
            "sunglasses" => Ok(Self::Sunglasses),
            _ => Err("Glasses must be off, round, square, or sunglasses".into()),
        }
    }
}
#[derive(Clone, Copy)]
pub struct Settings {
    pub style: Style,
    pub size: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            style: Style::Off,
            size: 1.0,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.size.is_finite() || !(0.6..=1.6).contains(&self.size) {
            Err("Glasses size must be 0.6..1.6".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy)]
struct Pixel {
    x: f32,
    y: f32,
}
fn pixel(point: Point, w: usize, h: usize) -> Pixel {
    Pixel {
        x: point.x * w as f32,
        y: point.y * h as f32,
    }
}
fn line(frame: &mut [u8], w: usize, h: usize, a: Pixel, b: Pixel, thickness: f32) {
    let radius = thickness * 0.5;
    let x0 = (a.x.min(b.x) - radius - 1.0).floor().max(0.0).min(w as f32) as usize;
    let x1 = (a.x.max(b.x) + radius + 1.0).ceil().max(0.0).min(w as f32) as usize;
    let y0 = (a.y.min(b.y) - radius - 1.0).floor().max(0.0).min(h as f32) as usize;
    let y1 = (a.y.max(b.y) + radius + 1.0).ceil().max(0.0).min(h as f32) as usize;
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = (dx * dx + dy * dy).max(0.001);
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let t = ((px - a.x) * dx + (py - a.y) * dy) / length;
            let t = t.clamp(0.0, 1.0);
            let distance = ((px - a.x - t * dx).powi(2) + (py - a.y - t * dy).powi(2)).sqrt();
            let alpha = (radius + 0.75 - distance).clamp(0.0, 1.0) * 0.94;
            if alpha <= 0.0 {
                continue;
            }
            let index = (y * w + x) * 3;
            for c in 0..3 {
                frame[index + c] = (frame[index + c] as f32 * (1.0 - alpha)
                    + [24.0, 28.0, 36.0][c] * alpha)
                    .round() as u8;
            }
        }
    }
}
fn dark_lens(
    frame: &mut [u8],
    width: usize,
    height: usize,
    center: Pixel,
    axis: Pixel,
    normal: Pixel,
    rx: f32,
    ry: f32,
) {
    let bound = (rx * rx + ry * ry).sqrt() + 1.0;
    let x0 = (center.x - bound).floor().clamp(0.0, width as f32) as usize;
    let x1 = (center.x + bound).ceil().clamp(0.0, width as f32) as usize;
    let y0 = (center.y - bound).floor().clamp(0.0, height as f32) as usize;
    let y1 = (center.y + bound).ceil().clamp(0.0, height as f32) as usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - center.x;
            let dy = y as f32 + 0.5 - center.y;
            let u = (dx * axis.x + dy * axis.y) / rx;
            let v = (dx * normal.x + dy * normal.y) / ry;
            let shape = u.powi(4) + v.powi(4);
            let coverage = ((1.0 - shape) * rx.min(ry) * 0.25 + 0.5).clamp(0.0, 1.0);
            if coverage == 0.0 {
                continue;
            }
            let alpha = coverage * 0.91;
            let top = (0.5 - v * 0.5).clamp(0.0, 1.0);
            let reflection = (1.0 - (v + 0.45 - u * 0.25).abs() / 0.12).clamp(0.0, 1.0) * 0.10;
            let i = (y * width + x) * 3;
            for c in 0..3 {
                let shade = [7.0, 9.0, 14.0][c]
                    + top * [13.0, 15.0, 19.0][c]
                    + reflection * [95.0, 105.0, 125.0][c];
                frame[i + c] = (frame[i + c] as f32 * (1.0 - alpha) + shade * alpha).round() as u8;
            }
        }
    }
}

/// Round/square lenses are transparent; sunglasses have a dark lens fill. No per-frame heap
/// allocations, no drawing without fresh landmarks, no effect when disabled.
pub fn render(
    frame: &mut [u8],
    width: usize,
    height: usize,
    mesh: Option<&Landmarks>,
    settings: Settings,
) {
    assert_eq!(frame.len(), width * height * 3);
    if settings.style == Style::Off || width == 0 || height == 0 {
        return;
    }
    let Some(mesh) = mesh else {
        return;
    };
    if mesh.points.len() != 468 {
        return;
    }
    let points = &mesh.points;
    let mut centers = [Pixel { x: 0.0, y: 0.0 }; 2];
    for (i, (a, b)) in [(33, 133), (362, 263)].into_iter().enumerate() {
        let a = pixel(points[a], width, height);
        let b = pixel(points[b], width, height);
        centers[i] = Pixel {
            x: (a.x + b.x) * 0.5,
            y: (a.y + b.y) * 0.5,
        };
    }
    if centers.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return;
    }
    let dx = centers[1].x - centers[0].x;
    let dy = centers[1].y - centers[0].y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance < 8.0 || distance > width as f32 * 0.75 {
        return;
    }
    let axis = Pixel {
        x: dx / distance,
        y: dy / distance,
    };
    let normal = Pixel {
        x: -axis.y,
        y: axis.x,
    };
    let rx = distance
        * if settings.style == Style::Round {
            0.36
        } else {
            0.40
        }
        * settings.size;
    let ry = distance
        * if settings.style == Style::Round {
            0.36
        } else {
            0.29
        }
        * settings.size;
    let thickness = (distance * 0.035).clamp(1.2, 5.0);
    let local = |center: Pixel, x: f32, y: f32| Pixel {
        x: center.x + axis.x * x + normal.x * y,
        y: center.y + axis.y * x + normal.y * y,
    };
    for center in centers {
        if settings.style == Style::Sunglasses {
            dark_lens(frame, width, height, center, axis, normal, rx, ry);
        }
        let at = |step: usize| {
            let theta = step as f32 / 64.0 * std::f32::consts::TAU;
            let (mut sy, mut sx) = theta.sin_cos();
            if settings.style != Style::Round {
                sx = sx.signum() * sx.abs().sqrt();
                sy = sy.signum() * sy.abs().sqrt();
            }
            local(center, sx * rx, sy * ry)
        };
        let mut last = at(0);
        for step in 1..=64 {
            let next = at(step);
            line(frame, width, height, last, next, thickness);
            last = next;
        }
    }
    // A shallow curved bridge joins the inner sides of the two frames.
    let a = local(centers[0], rx * 0.98, -ry * 0.18);
    let b = local(centers[1], -rx * 0.98, -ry * 0.18);
    let middle = Pixel {
        x: (a.x + b.x) * 0.5 - normal.x * ry * 0.18,
        y: (a.y + b.y) * 0.5 - normal.y * ry * 0.18,
    };
    let mut last = a;
    for step in 1..=12 {
        let t = step as f32 / 12.0;
        let q = 1.0 - t;
        let next = Pixel {
            x: q * q * a.x + 2.0 * q * t * middle.x + t * t * b.x,
            y: q * q * a.y + 2.0 * q * t * middle.y + t * t * b.y,
        };
        line(frame, width, height, last, next, thickness * 0.8);
        last = next;
    }
    for (i, sign) in [(0, -1.0), (1, 1.0)] {
        let start = local(centers[i], sign * rx * 0.98, -ry * 0.18);
        let end = local(
            centers[i],
            sign * (rx + distance * 0.14),
            -ry * 0.18 - distance * 0.05,
        );
        line(frame, width, height, start, end, thickness * 0.8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mesh(tilt: bool) -> Landmarks {
        let mut points = vec![Point { x: 0.5, y: 0.5 }; 468];
        for (index, x, y) in [
            (33, 0.30, 0.4),
            (133, 0.40, 0.4),
            (362, 0.60, 0.4),
            (263, 0.70, 0.4),
        ] {
            points[index] = Point {
                x,
                y: if tilt { y + (x - 0.5) * 0.5 } else { y },
            };
        }
        Landmarks { points }
    }
    #[test]
    fn missing_disabled_and_invalid_landmarks_leave_frame_unchanged() {
        let original = vec![180; 200 * 200 * 3];
        let mut frame = original.clone();
        render(
            &mut frame,
            200,
            200,
            Some(&mesh(false)),
            Settings::default(),
        );
        assert_eq!(frame, original);
        let settings = Settings {
            style: Style::Round,
            ..Settings::default()
        };
        render(&mut frame, 200, 200, None, settings);
        assert_eq!(frame, original);
        render(
            &mut frame,
            200,
            200,
            Some(&Landmarks { points: vec![] }),
            settings,
        );
        assert_eq!(frame, original);
    }
    #[test]
    fn frames_draw_while_lens_centers_stay_transparent() {
        for style in [Style::Round, Style::Square] {
            let mut frame = vec![180; 200 * 200 * 3];
            render(
                &mut frame,
                200,
                200,
                Some(&mesh(false)),
                Settings { style, size: 1.0 },
            );
            assert!(frame.iter().any(|&v| v < 100));
            for x in [70, 130] {
                assert_eq!(
                    &frame[(80 * 200 + x) * 3..(80 * 200 + x) * 3 + 3],
                    &[180; 3]
                );
            }
            assert_eq!(&frame[..3], &[180; 3]);
        }
    }
    #[test]
    fn tilt_changes_orientation_and_validation_rejects_bad_options() {
        let settings = Settings {
            style: Style::Round,
            ..Settings::default()
        };
        let mut a = vec![180; 200 * 200 * 3];
        let mut b = a.clone();
        render(&mut a, 200, 200, Some(&mesh(false)), settings);
        render(&mut b, 200, 200, Some(&mesh(true)), settings);
        assert_ne!(a, b);
        assert!(Style::parse("unknown").is_err());
        assert!(Settings {
            size: f32::NAN,
            ..settings
        }
        .validate()
        .is_err());
    }
}

#[cfg(test)]
mod sunglasses_tests {
    use super::*;
    #[test]
    fn sunglasses_darken_lens_centers_without_changing_background() {
        let mut points = vec![Point { x: 0.5, y: 0.5 }; 468];
        for (i, x) in [(33, 0.30), (133, 0.40), (362, 0.60), (263, 0.70)] {
            points[i] = Point { x, y: 0.4 };
        }
        let mesh = Landmarks { points };
        let mut frame = vec![180; 200 * 200 * 3];
        let settings = Settings {
            style: Style::Sunglasses,
            size: 1.0,
        };
        render(&mut frame, 200, 200, Some(&mesh), settings);
        for x in [70, 130] {
            assert!(frame[(80 * 200 + x) * 3] < 60);
        }
        assert_eq!(&frame[..3], &[180; 3]);
        let mut absent = vec![180; frame.len()];
        render(&mut absent, 200, 200, None, settings);
        assert!(absent.iter().all(|&v| v == 180));
        assert_eq!(Style::parse("sunglasses").unwrap(), Style::Sunglasses);
    }
}
