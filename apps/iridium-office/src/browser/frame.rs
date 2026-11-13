//! The frame pipeline: the page becomes an egui texture.
//!
//! Chromium pushes a screenshot of the page as a screencast frame. This turns
//! that into pixels the suite can draw.
//!
//! The split matters:
//!
//! - The **worker** owns the decoding. Decoding a JPEG is milliseconds of
//!   work, and doing it on the UI thread would stall the whole suite.
//! - The **UI thread** only ever takes the newest finished frame and hands it
//!   to egui. It never waits.
//!
//! Only the newest frame matters. A frame that arrives while an older one is
//! still being drawn is stale by the time it would be shown, so it is dropped
//! rather than queued. That keeps memory flat no matter how fast the page
//! changes.

use std::sync::{Arc, Mutex};

use egui::ColorImage;

/// The widest and tallest a frame may be, in pixels.
///
/// A browser window is never this big, so anything larger is a mistake or an
/// attack, and is refused rather than decoded.
pub const MAX_FRAME_WIDTH: u32 = 4096;
/// The tallest a frame may be, in pixels.
pub const MAX_FRAME_HEIGHT: u32 = 4096;

/// The most a decoded frame may allocate, in bytes.
///
/// Four thousand by four thousand pixels of RGBA is 64 MB. Anything asking for
/// more is refused before the decoder allocates it.
pub const MAX_FRAME_BYTES: u64 = 64 * 1024 * 1024;

/// A decoded frame, ready to draw.
#[derive(Clone)]
pub struct Frame {
    /// The pixels, four bytes each, row by row.
    pub pixels: Vec<u8>,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

impl Frame {
    /// The frame as an egui image.
    pub fn as_color_image(&self) -> ColorImage {
        ColorImage::from_rgba_unmultiplied(
            [self.width as usize, self.height as usize],
            &self.pixels,
        )
    }
}

/// The newest finished frame, shared between the worker and the UI.
#[derive(Clone, Default)]
pub struct FrameSlot(Arc<Mutex<Option<Frame>>>);

impl FrameSlot {
    /// An empty slot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Put a frame in, replacing any older one.
    pub fn put(&self, frame: Frame) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(frame);
    }

    /// Take the newest frame, if there is one.
    ///
    /// The frame is taken, not copied: the UI thread has it, and the worker
    /// will not hand the same pixels over twice.
    pub fn take(&self) -> Option<Frame> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.take()
    }

    /// Whether a frame is waiting.
    pub fn has_frame(&self) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }
}

/// Decode a screencast frame's data into pixels.
///
/// The data is base64-encoded JPEG or PNG. The format is not guessed: the
/// browser is told which one to send, and this decodes what it was given.
pub fn decode(data: &str, format: FrameFormat) -> Option<Frame> {
    use base64::Engine as _;

    // Refuse a frame that is absurdly large before spending memory on it.
    // Base64 is four characters for three bytes.
    if data.len() > (MAX_FRAME_BYTES as usize) * 2 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_FRAME_WIDTH);
    limits.max_image_height = Some(MAX_FRAME_HEIGHT);
    limits.max_alloc = Some(MAX_FRAME_BYTES);

    let mut reader = match format {
        FrameFormat::Jpeg => {
            image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Jpeg)
        }
        FrameFormat::Png => {
            image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
        }
    };
    // The limits are set on the reader, not chained: `limits` takes `&mut self`.
    reader.limits(limits);

    let decoded = reader.decode().ok()?;
    let (width, height) = (decoded.width(), decoded.height());
    if width == 0 || height == 0 || width > MAX_FRAME_WIDTH || height > MAX_FRAME_HEIGHT {
        return None;
    }
    // The decoded size is checked separately: the decoder's own limits are
    // not a promise about the final image.
    let rgba = decoded.to_rgba8();
    if rgba.len() as u64 > MAX_FRAME_BYTES {
        return None;
    }
    Some(Frame {
        pixels: rgba.into_raw(),
        width,
        height,
    })
}

/// Which image format the browser is sending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    /// JPEG. Smaller and faster to decode; the default.
    Jpeg,
    /// PNG. Lossless, and slower.
    Png,
}

impl FrameFormat {
    /// The name the browser protocol uses.
    pub fn as_protocol(self) -> &'static str {
        match self {
            FrameFormat::Jpeg => "jpeg",
            FrameFormat::Png => "png",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    /// A frame survives the round trip through the slot.
    #[test]
    fn a_frame_is_delivered_once() {
        let slot = FrameSlot::new();
        assert!(!slot.has_frame());
        slot.put(Frame {
            pixels: vec![1, 2, 3, 4],
            width: 1,
            height: 1,
        });
        assert!(slot.has_frame());
        let frame = slot.take().expect("the frame");
        assert_eq!(frame.pixels, vec![1, 2, 3, 4]);
        // Taken means taken: the same pixels are not handed over twice.
        assert!(slot.take().is_none());
    }

    /// Only the newest frame survives; an older one is dropped.
    #[test]
    fn only_the_newest_frame_is_kept() {
        let slot = FrameSlot::new();
        slot.put(Frame {
            pixels: vec![1],
            width: 1,
            height: 1,
        });
        slot.put(Frame {
            pixels: vec![2],
            width: 1,
            height: 1,
        });
        let frame = slot.take().expect("a frame");
        assert_eq!(frame.pixels, vec![2], "the newer frame wins");
    }

    /// A real JPEG decodes to the right pixels.
    #[test]
    fn a_jpeg_decodes() {
        // A 2x2 JPEG, made by the browser's own encoder in the probe run.
        let jpeg = include_bytes!("testdata/tiny.jpg");
        let encoded = base64::engine::general_purpose::STANDARD.encode(jpeg);
        let frame = decode(&encoded, FrameFormat::Jpeg).expect("the frame decodes");
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.pixels.len(), 2 * 2 * 4);
    }

    /// Rubbish in, nothing out. A page cannot make the suite allocate.
    #[test]
    fn rubbish_does_not_decode() {
        assert!(decode("not base64 at all", FrameFormat::Jpeg).is_none());
        assert!(decode("", FrameFormat::Png).is_none());
        let encoded = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        assert!(decode(&encoded, FrameFormat::Png).is_none());
    }
}
