//! Video → vector.
//!
//! Sample frames with ffmpeg, embed each through the image embedder, then
//! temporal-pool (mean + L2-normalize) into one clip vector in the *same* shared
//! space as text and images — so a video is directly comparable / classifiable.
//!
//! Fail-soft by design: missing ffmpeg, a non-image embedder (e.g. text-only
//! BGE), or zero decoded frames all return `None` rather than erroring. The
//! audio track is out of scope here — route that through the voice pipeline.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::embed::VectorEmbed;

#[derive(Debug, Clone)]
pub struct VideoEmbedOptions {
    /// Frames sampled per second of video (ffmpeg `fps` filter).
    pub fps: f32,
    /// Hard cap on the number of frames embedded.
    pub max_frames: usize,
}

impl Default for VideoEmbedOptions {
    fn default() -> Self {
        Self {
            fps: 1.0,
            max_frames: 32,
        }
    }
}

pub struct VideoEmbedResult {
    pub embedding: Vec<f32>,
    /// Frames that actually contributed to the pooled vector.
    pub frames: usize,
}

/// Embed a video file. `None` when the embedder has no image path, ffmpeg is
/// unavailable / fails, or no frame produced a vector.
pub fn embed_video_file(
    path: &Path,
    embedder: &dyn VectorEmbed,
    opts: &VideoEmbedOptions,
) -> Option<VideoEmbedResult> {
    let work = std::env::temp_dir().join(format!("physis_vid_{}", uuid::Uuid::new_v4()));
    if std::fs::create_dir_all(&work).is_err() {
        return None;
    }
    let _work_guard = TempDir(work.clone()); // RAII cleanup of extracted frames

    let pattern = work.join("f_%05d.jpg");
    let vf = format!("fps={}", opts.fps.max(0.01));
    let status = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-vf",
            &vf,
            "-frames:v",
            &opts.max_frames.to_string(),
            "-q:v",
            "3",
        ])
        .arg(&pattern)
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(_) => {
            log::warn!("[video] ffmpeg exited non-zero for {}", path.display());
            return None;
        }
        Err(e) => {
            log::warn!("[video] ffmpeg unavailable: {e}");
            return None;
        }
    }

    // Frame files in chronological order (zero-padded names sort correctly).
    let mut frames: Vec<PathBuf> = std::fs::read_dir(&work)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jpg"))
        .collect();
    frames.sort();
    if frames.is_empty() {
        return None;
    }

    let dim = embedder.dimension();
    let mut acc = vec![0.0f32; dim];
    let mut n = 0usize;
    for f in frames.iter().take(opts.max_frames) {
        let Ok(bytes) = std::fs::read(f) else {
            continue;
        };
        if let Some(v) = embedder.embed_image(&bytes) {
            if v.len() == dim {
                for i in 0..dim {
                    acc[i] += v[i];
                }
                n += 1;
            }
        }
    }
    if n == 0 {
        return None;
    }

    // Mean-pool → L2-normalize → unit vector in the shared embedding space.
    for x in acc.iter_mut() {
        *x /= n as f32;
    }
    let norm = acc.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    for x in acc.iter_mut() {
        *x /= norm;
    }
    Some(VideoEmbedResult {
        embedding: acc,
        frames: n,
    })
}

/// Same as [`embed_video_file`] but from in-memory bytes (writes a temp file
/// first; `ext` is the container hint, e.g. "mp4"/"webm").
pub fn embed_video_bytes(
    bytes: &[u8],
    ext: &str,
    embedder: &dyn VectorEmbed,
    opts: &VideoEmbedOptions,
) -> Option<VideoEmbedResult> {
    let safe: String = ext
        .trim_start_matches('.')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    let ext = if safe.is_empty() {
        "mp4".to_string()
    } else {
        safe
    };
    let tmp = std::env::temp_dir().join(format!("physis_vid_{}.{}", uuid::Uuid::new_v4(), ext));
    if std::fs::write(&tmp, bytes).is_err() {
        return None;
    }
    let _file_guard = TempFile(tmp.clone());
    embed_video_file(&tmp, embedder, opts)
}

// ── RAII temp cleanup ───────────────────────────────────────────────────────
struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text-only embedder: `embed_image` keeps the trait default (`None`).
    struct TextOnly;
    impl VectorEmbed for TextOnly {
        fn embed(&self, _text: &str) -> Vec<f32> {
            vec![0.0; 4]
        }
        fn dimension(&self) -> usize {
            4
        }
    }

    // These exercise the documented fail-soft contract: every failure path
    // (missing file, undecodable bytes, hostile extension, absent ffmpeg)
    // returns None and never panics. They hold whether or not ffmpeg is
    // installed, since a missing binary is itself one of the guarded paths.

    #[test]
    fn missing_file_returns_none() {
        let opts = VideoEmbedOptions::default();
        let result = embed_video_file(Path::new("/nonexistent/clip.mp4"), &TextOnly, &opts);
        assert!(result.is_none());
    }

    #[test]
    fn undecodable_bytes_return_none() {
        let opts = VideoEmbedOptions::default();
        let result = embed_video_bytes(b"definitely not a video", "mp4", &TextOnly, &opts);
        assert!(result.is_none());
    }

    #[test]
    fn hostile_extension_is_sanitized_not_trusted() {
        // Path-traversal / shell metacharacters in `ext` must not escape the
        // temp dir or panic; the call just fails soft on the garbage payload.
        let opts = VideoEmbedOptions::default();
        for ext in ["../../etc/passwd", "mp4; rm -rf /", "", ".."] {
            let result = embed_video_bytes(b"junk", ext, &TextOnly, &opts);
            assert!(result.is_none(), "ext {ext:?} should fail soft");
        }
    }

    #[test]
    fn default_options_are_sane() {
        let opts = VideoEmbedOptions::default();
        assert!(opts.fps > 0.0);
        assert!(opts.max_frames > 0);
    }
}
