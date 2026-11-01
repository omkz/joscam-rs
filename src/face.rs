//! Background face detection and optional dense landmarks; no identity recognition.
use std::{
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
struct Frame {
    gray: Vec<u8>,
    rgb: Vec<u8>,
    rgb_width: usize,
    rgb_height: usize,
    width: u32,
    height: u32,
    captured: Instant,
}
struct Detection {
    rect: Option<FaceRect>,
    landmarks: Option<crate::landmarks::Landmarks>,
    captured: Instant,
}

pub struct FaceTracker {
    input: Option<SyncSender<Frame>>,
    results: Receiver<Detection>,
    worker: Option<JoinHandle<()>>,
    next_submit: Instant,
    updated: Option<Instant>,
    rect: Option<FaceRect>,
    landmarks: Option<crate::landmarks::Landmarks>,
}
impl FaceTracker {
    #[cfg(test)]
    pub fn new(path: &str) -> Result<Self, String> {
        Self::with_landmarks(path, None)
    }
    pub fn with_landmarks(path: &str, mesh_path: Option<&str>) -> Result<Self, String> {
        let mesh = mesh_path.map(crate::landmarks::FaceMesh::new).transpose()?;
        // Load before opening the webcam so a missing model is actionable.
        let model = rustface::load_model(path).map_err(|e| {
            format!("Cannot load face model {path}: {e}. Run: bash scripts/download-face-model.sh")
        })?;
        let (tx, rx) = mpsc::sync_channel::<Frame>(1);
        let (out_tx, out_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut warned = false;
            let mut detector = rustface::create_detector_with_model(model);
            detector.set_min_face_size(30);
            detector.set_score_thresh(2.5);
            detector.set_pyramid_scale_factor(0.8);
            detector.set_slide_window_step(4, 4);
            while let Ok(mut frame) = rx.recv() {
                // Prefer the newest queued frame, never work through a backlog.
                while let Ok(newer) = rx.try_recv() {
                    frame = newer;
                }
                let image = rustface::ImageData::new(&frame.gray, frame.width, frame.height);
                let faces = detector.detect(&image);
                let rect = faces
                    .iter()
                    .max_by_key(|f| f.bbox().width() as u64 * f.bbox().height() as u64)
                    .map(|f| {
                        let b = f.bbox();
                        FaceRect {
                            x: b.x() as f32 / frame.width as f32,
                            y: b.y() as f32 / frame.height as f32,
                            w: b.width() as f32 / frame.width as f32,
                            h: b.height() as f32 / frame.height as f32,
                        }
                    });
                let landmarks = if let (Some(mesh), Some(rect)) = (&mesh, rect) {
                    match mesh.infer(&frame.rgb, frame.rgb_width, frame.rgb_height, rect) {
                        Ok(points) => points,
                        Err(error) => {
                            if !warned {
                                eprintln!("Landmark inference failed: {error}");
                                warned = true;
                            }
                            None
                        }
                    }
                } else {
                    None
                };
                if out_tx
                    .send(Detection {
                        rect,
                        landmarks,
                        captured: frame.captured,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            input: Some(tx),
            results: out_rx,
            worker: Some(worker),
            next_submit: Instant::now(),
            updated: None,
            rect: None,
            landmarks: None,
        })
    }
    pub fn update(&mut self, rgb: &[u8], width: usize, height: usize) -> Option<FaceRect> {
        let now = Instant::now();
        while let Ok(result) = self.results.try_recv() {
            self.landmarks = if now.duration_since(result.captured) <= Duration::from_millis(250) {
                result.landmarks
            } else {
                None
            };
            self.rect = if now.duration_since(result.captured) <= Duration::from_millis(350) {
                match (self.rect, result.rect) {
                    (Some(old), Some(new))
                        if (old.x - new.x).abs() + (old.y - new.y).abs() < 0.2 =>
                    {
                        let lerp = |a: f32, b: f32| a * 0.35 + b * 0.65;
                        Some(FaceRect {
                            x: lerp(old.x, new.x),
                            y: lerp(old.y, new.y),
                            w: lerp(old.w, new.w),
                            h: lerp(old.h, new.h),
                        })
                    }
                    (_, new) => new,
                }
            } else {
                None
            };
            self.updated = Some(result.captured);
        }
        if self
            .updated
            .map_or(true, |t| now.duration_since(t) > Duration::from_millis(350))
        {
            self.rect = None;
            self.landmarks = None;
        }
        if self
            .updated
            .map_or(true, |t| now.duration_since(t) > Duration::from_millis(250))
        {
            self.landmarks = None;
        }
        if now >= self.next_submit {
            let w = width.min(256);
            let h = (height * w / width).max(1);
            let mut gray = vec![0; w * h];
            for y in 0..h {
                for x in 0..w {
                    let i = ((y * height / h) * width + x * width / w) * 3;
                    gray[y * w + x] =
                        ((77 * rgb[i] as u32 + 150 * rgb[i + 1] as u32 + 29 * rgb[i + 2] as u32)
                            >> 8) as u8;
                }
            }
            if let Some(tx) = &self.input {
                match tx.try_send(Frame {
                    gray,
                    rgb: rgb.to_vec(),
                    rgb_width: width,
                    rgb_height: height,
                    width: w as u32,
                    height: h as u32,
                    captured: now,
                }) {
                    Ok(()) | Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Disconnected(_)) => {
                        self.rect = None;
                        self.updated = None;
                        self.landmarks = None;
                    }
                }
            }
            self.next_submit = now + Duration::from_millis(100);
        }
        if self.rect.is_none() {
            self.landmarks = None;
        }
        self.rect
    }
    pub fn landmarks(&self) -> Option<&crate::landmarks::Landmarks> {
        self.landmarks.as_ref()
    }
}
impl Drop for FaceTracker {
    fn drop(&mut self) {
        self.input.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_model_has_actionable_error() {
        assert!(FaceTracker::new("/missing/joscam-model.bin")
            .err()
            .unwrap()
            .contains("download-face-model"));
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn stale_face_is_discarded() {
        let (_tx, rx) = mpsc::channel();
        let mut tracker = FaceTracker {
            input: None,
            results: rx,
            worker: None,
            next_submit: Instant::now() + Duration::from_secs(5),
            updated: Some(Instant::now() - Duration::from_secs(1)),
            landmarks: None,
            rect: Some(FaceRect {
                x: 0.1,
                y: 0.1,
                w: 0.3,
                h: 0.4,
            }),
        };
        assert_eq!(tracker.update(&[0; 12], 2, 2), None);
    }
    #[test]
    #[ignore = "requires downloaded model and RGB fixture; see README"]
    fn real_model_detects_face_then_clears_blank_frame() {
        let model = std::env::var("JOSCAM_TEST_MODEL").unwrap();
        let fixture = std::fs::read(std::env::var("JOSCAM_TEST_RGB").unwrap()).unwrap();
        let (w, h) = (640, 436);
        assert_eq!(fixture.len(), w * h * 3);
        let mut tracker = FaceTracker::new(&model).unwrap();
        let started = Instant::now();
        let mut detected = None;
        while started.elapsed() < Duration::from_secs(3) {
            detected = tracker.update(&fixture, w, h);
            if detected.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        eprintln!("Real detection: {:?} in {:?}", detected, started.elapsed());
        assert!(detected.is_some());
        let blank = vec![0; fixture.len()];
        let started = Instant::now();
        loop {
            if tracker.update(&blank, w, h).is_none() {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_millis(10));
        }
    }
}
