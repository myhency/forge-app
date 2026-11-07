//! `gif_creator` of the Browser pane tools: while a recording runs, the tab is pictured about once a
//! second (an unchanged picture is skipped, at most `MAX_FRAMES`), and `export` writes the pictures as
//! a looping GIF into the Downloads folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const MAX_FRAMES: usize = 60;
/// Frames wider than this are scaled down: a GIF of full-size screenshots gets large quickly.
const MAX_WIDTH: u32 = 800;
const FRAME_DELAY_MS: u32 = 700;

#[derive(Default)]
pub struct Recording {
    pub tab: u64,
    pub running: bool,
    /// Bumped by every start, so the capture loop of an earlier recording stops.
    pub generation: u64,
    pub frames: Vec<Vec<u8>>,
}

/// The recordings by session (the session's id as text).
pub fn recordings() -> &'static Mutex<HashMap<String, Recording>> {
    static RECORDINGS: OnceLock<Mutex<HashMap<String, Recording>>> = OnceLock::new();
    RECORDINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Adds a picture unless it is the same as the last one or the recording is full. True when added.
pub fn push_frame(recording: &mut Recording, png: Vec<u8>) -> bool {
    if recording.frames.len() >= MAX_FRAMES || recording.frames.last() == Some(&png) {
        return false;
    }
    recording.frames.push(png);
    true
}

/// The pictures as one looping GIF, every frame the size of the first (scaled to at most `MAX_WIDTH`).
pub fn encode_gif(frames: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame};
    let first = frames.first().ok_or_else(|| "Nothing was recorded.".to_string())?;
    let first = image::load_from_memory(first).map_err(|error| format!("A recorded picture cannot be read: {error}"))?;
    let (width, height) = if first.width() > MAX_WIDTH {
        (MAX_WIDTH, (first.height() as u64 * MAX_WIDTH as u64 / first.width() as u64).max(1) as u32)
    } else {
        (first.width(), first.height())
    };
    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new_with_speed(&mut out, 10);
        encoder.set_repeat(Repeat::Infinite).map_err(|error| error.to_string())?;
        for png in frames {
            let picture = image::load_from_memory(png).map_err(|error| format!("A recorded picture cannot be read: {error}"))?;
            let rgba = image::imageops::resize(&picture.to_rgba8(), width, height, image::imageops::FilterType::Triangle);
            encoder.encode_frame(Frame::from_parts(rgba, 0, 0, Delay::from_numer_denom_ms(FRAME_DELAY_MS, 1))).map_err(|error| error.to_string())?;
        }
    }
    Ok(out)
}

/// The Downloads folder in `home`, or `fallback` when there is none.
pub fn downloads_folder(home: Option<PathBuf>, fallback: PathBuf) -> PathBuf {
    home.map(|home| home.join("Downloads")).filter(|folder| folder.is_dir()).unwrap_or(fallback)
}

/// A file name in `folder` that is not taken: `name.gif`, then `name (2).gif`… The name is cut down
/// to letters, digits, spaces, `-`, `_` and `.`.
pub fn free_gif_path(folder: &Path, name: &str) -> PathBuf {
    let cleaned: String = name.trim().trim_end_matches(".gif").chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).take(80).collect();
    let stem = match cleaned.trim().trim_start_matches('.') {
        "" => "recording".to_string(),
        kept => kept.to_string(),
    };
    let mut path = folder.join(format!("{stem}.gif"));
    let mut number = 2;
    while path.exists() {
        path = folder.join(format!("{stem} ({number}).gif"));
        number += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(colour: [u8; 4], width: u32) -> Vec<u8> {
        let picture = image::RgbaImage::from_pixel(width, width / 2, image::Rgba(colour));
        let mut out = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn a_same_picture_is_skipped_and_the_recording_has_a_limit() {
        let mut recording = Recording::default();
        assert!(push_frame(&mut recording, vec![1]));
        assert!(!push_frame(&mut recording, vec![1]));
        assert!(push_frame(&mut recording, vec![2]));
        for n in 0..100u8 {
            push_frame(&mut recording, vec![3, n]);
        }
        assert_eq!(recording.frames.len(), MAX_FRAMES);
    }

    #[test]
    fn frames_become_a_looping_gif_of_the_first_frames_size() {
        let gif = encode_gif(&[png([255, 0, 0, 255], 1600), png([0, 0, 255, 255], 400)]).unwrap();
        assert_eq!(&gif[..6], b"GIF89a");
        let decoded = image::load_from_memory(&gif).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (800, 400));
        assert_eq!(encode_gif(&[]).unwrap_err(), "Nothing was recorded.");
    }

    #[test]
    fn a_gif_name_is_cleaned_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("forge-gif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_gif_path(&dir, "../../etc/x"), dir.join("etcx.gif"), "no folder parts, no leading dots");
        assert_eq!(free_gif_path(&dir, ""), dir.join("recording.gif"));
        std::fs::write(dir.join("demo.gif"), b"x").unwrap();
        assert_eq!(free_gif_path(&dir, "demo.gif"), dir.join("demo (2).gif"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
