//! Local 468-point Face Mesh inference. RGB crops are normalized to [0, 1].
use crate::face::FaceRect;
use ort::{Session, Tensor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Landmarks {
    pub points: Vec<Point>,
}
pub struct FaceMesh {
    model: Session,
}
impl FaceMesh {
    pub fn new(path: &str) -> Result<Self, String> {
        eprintln!("Loading local 468-point landmark model...");
        let runtime = std::env::var("ORT_DYLIB_PATH")
            .unwrap_or_else(|_| "models/onnxruntime/lib/libonnxruntime.so.1.19.2".into());
        if !std::path::Path::new(&runtime).is_file() {
            return Err(format!(
                "ONNX Runtime missing at {runtime}. Run: bash scripts/setup-beauty.sh"
            ));
        }
        ort::init_from(runtime)
            .with_name("Joscam")
            .commit()
            .map_err(|e| e.to_string())?;
        let model = Session::builder()
            .and_then(|s| s.with_intra_threads(1))
            .and_then(|s| s.with_inter_threads(1))
            .and_then(|s| s.commit_from_file(path))
            .map_err(|e| {
                format!("Cannot load landmark model {path}: {e}. Run: bash scripts/setup-beauty.sh")
            })?;
        Ok(Self { model })
    }
    pub fn infer(
        &self,
        rgb: &[u8],
        width: usize,
        height: usize,
        face: FaceRect,
    ) -> Result<Option<Landmarks>, String> {
        let side = (face.w * width as f32).max(face.h * height as f32) * 1.5;
        if side < 4.0 || !side.is_finite() {
            return Ok(None);
        }
        let x0 = (face.x + face.w * 0.5) * width as f32 - side * 0.5;
        let y0 = (face.y + face.h * 0.5) * height as f32 - side * 0.5;
        let mut values = vec![0.0f32; 3 * 192 * 192];
        for y in 0..192 {
            for x in 0..192 {
                let fx = x0 + x as f32 * side / 192.0;
                let fy = y0 + y as f32 * side / 192.0;
                if fx < 0.0 || fy < 0.0 || fx >= width as f32 - 1.0 || fy >= height as f32 - 1.0 {
                    continue;
                }
                let ix = fx as usize;
                let iy = fy as usize;
                let dx = fx - ix as f32;
                let dy = fy - iy as f32;
                for c in 0..3 {
                    let at = |xx, yy| rgb[(yy * width + xx) * 3 + c] as f32;
                    let v = at(ix, iy) * (1.0 - dx) * (1.0 - dy)
                        + at(ix + 1, iy) * dx * (1.0 - dy)
                        + at(ix, iy + 1) * (1.0 - dx) * dy
                        + at(ix + 1, iy + 1) * dx * dy;
                    values[c * 192 * 192 + y * 192 + x] = v / 255.0;
                }
            }
        }
        let input = Tensor::from_array(([1usize, 3, 192, 192], values.into_boxed_slice()))
            .map_err(|e| e.to_string())?;
        let output = self
            .model
            .run(ort::inputs![input].map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if output.len() != 2 {
            return Err("Unexpected landmark model outputs".into());
        }
        let (_, score) = output[1]
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        if score.is_empty() || !score[0].is_finite() || score[0] < 1.1 {
            return Ok(None);
        }
        let (_, points) = output[0]
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        if points.len() != 468 * 3 {
            return Err("Expected 468 facial landmarks".into());
        }
        let points: Vec<_> = points
            .chunks_exact(3)
            .map(|p| Point {
                x: (x0 + p[0] * side / 192.0) / width as f32,
                y: (y0 + p[1] * side / 192.0) / height as f32,
            })
            .collect();
        if points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || p.x < -0.2
                || p.x > 1.2
                || p.y < -0.2
                || p.y > 1.2
        }) {
            return Ok(None);
        }
        // Reject inconsistent geometry rather than painting features blindly.
        if points[33].x >= points[263].x
            || points[61].x >= points[291].x
            || points[13].y < points[33].y.min(points[263].y)
        {
            return Ok(None);
        }
        Ok(Some(Landmarks { points }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires local ONNX model and 640x436 RGB fixture"]
    fn real_landmarks_have_valid_eye_and_lip_geometry() {
        let path = std::env::var("JOSCAM_TEST_MESH").unwrap();
        let rgb = std::fs::read(std::env::var("JOSCAM_TEST_RGB").unwrap()).unwrap();
        let (w, h) = (640, 436);
        assert_eq!(rgb.len(), w * h * 3);
        let gray: Vec<_> = rgb
            .chunks_exact(3)
            .map(|p| ((77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8) as u8)
            .collect();
        let mut detector =
            rustface::create_detector(&std::env::var("JOSCAM_TEST_MODEL").unwrap()).unwrap();
        detector.set_min_face_size(60);
        detector.set_score_thresh(2.0);
        detector.set_slide_window_step(4, 4);
        detector.set_pyramid_scale_factor(0.8);
        let detected = detector.detect(&rustface::ImageData::new(&gray, w as u32, h as u32));
        let f = detected
            .iter()
            .max_by_key(|f| f.bbox().width() * f.bbox().height())
            .unwrap()
            .bbox();
        let rect = FaceRect {
            x: f.x() as f32 / w as f32,
            y: f.y() as f32 / h as f32,
            w: f.width() as f32 / w as f32,
            h: f.height() as f32 / h as f32,
        };
        let started = std::time::Instant::now();
        let model = FaceMesh::new(&path).unwrap();
        eprintln!("Mesh model load/optimization: {:?}", started.elapsed());
        let started = std::time::Instant::now();
        let mesh = model.infer(&rgb, w, h, rect).unwrap().unwrap();
        eprintln!("Mesh inference: {:?}", started.elapsed());
        assert_eq!(mesh.points.len(), 468);
        assert!(mesh.points[61].x < mesh.points[291].x);
        assert!(mesh.points[13].y > mesh.points[33].y);
        if let Ok(path) = std::env::var("JOSCAM_TEST_RENDER") {
            let mut beauty = crate::beauty::Beauty::default();
            beauty.skin_brightening = 0.65;
            beauty.rosy = 0.25;
            beauty.glow = 0.4;
            beauty.lip_tint = 0.5;
            beauty.eye_brightening = 0.5;
            beauty.set_tracked_face(Some(rect));
            beauty.set_landmarks(Some(mesh.clone()));
            let mut output = rgb.clone();
            beauty.process(&rgb, &mut output, w, h, 0.85, 0.35);
            std::fs::write(path, &output).unwrap();
        }
        if let Ok(path) = std::env::var("JOSCAM_TEST_DUMP") {
            let data = mesh
                .points
                .iter()
                .map(|p| format!("{},{}\n", p.x, p.y))
                .collect::<String>();
            std::fs::write(path, data).unwrap();
        }
    }
}
