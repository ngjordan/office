//! The screencast worker: frames from the browser, pixels for the suite.
//!
//! Chromium pushes a frame whenever the page changes. The worker listens for
//! those frames, decodes them, and leaves the newest one in a slot for the UI
//! thread to pick up.
//!
//! The worker owns the decoding because that is the expensive part. The UI
//! thread never blocks on a frame: it takes whatever is ready and draws it.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use headless_chrome::browser::transport::{SessionId, Transport};
use headless_chrome::protocol::cdp::types::Event;
use headless_chrome::protocol::cdp::{Page, Page::StartScreencastFormatOption};

use super::frame::{Frame, FrameFormat, FrameSlot};

/// How the screencast runs.
pub struct Screencast {
    /// The newest decoded frame, for the UI thread to take.
    pub frames: FrameSlot,
    /// The worker's thread. Dropping the screencast stops the frames.
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Set when the worker has stopped, with the reason.
    stopped: Arc<Mutex<Option<String>>>,
}

impl Screencast {
    /// Start casting the page.
    ///
    /// `max_width` and `max_height` cap the frame: the browser scales the
    /// page down to fit, so a small cap costs sharpness and a large one costs
    /// memory and decoding time.
    pub fn start(
        transport: Arc<Transport>,
        session: SessionId,
        events: Receiver<Event>,
        format: FrameFormat,
        max_width: u32,
        max_height: u32,
    ) -> Result<Self, String> {
        // The browser must be told to start before the frames arrive.
        transport
            .call_method_on_target(
                session.clone(),
                Page::StartScreencast {
                    format: Some(match format {
                        FrameFormat::Jpeg => StartScreencastFormatOption::Jpeg,
                        FrameFormat::Png => StartScreencastFormatOption::Png,
                    }),
                    quality: Some(80),
                    max_width: Some(max_width),
                    max_height: Some(max_height),
                    every_nth_frame: Some(1),
                },
            )
            .map_err(|e| format!("cannot start the screencast: {e}"))?;

        let frames = FrameSlot::new();
        let stopped: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let worker_frames = frames.clone();
        let worker_stopped = stopped.clone();
        let worker = std::thread::Builder::new()
            .name("workspace-browser-frames".into())
            .spawn(move || {
                while let Ok(event) = events.recv() {
                    let Event::PageScreencastFrame(frame) = event else {
                        continue;
                    };
                    // Every frame must be acknowledged, or the browser stops
                    // sending them. This happens even for a frame we drop.
                    let _ = transport.call_method_on_target(
                        session.clone(),
                        Page::ScreencastFrameAck {
                            session_id: frame.params.session_id,
                        },
                    );
                    // A frame that is too big is dropped, not drawn.
                    if let Some(decoded) = super::frame::decode(&frame.params.data, format) {
                        worker_frames.put(decoded);
                    }
                }
                *worker_stopped.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some("the browser connection closed".to_string());
            })
            .map_err(|e| format!("cannot start the frame worker: {e}"))?;

        Ok(Self {
            frames,
            worker: Mutex::new(Some(worker)),
            stopped,
        })
    }

    /// The newest frame, if there is one.
    pub fn take_frame(&self) -> Option<Frame> {
        self.frames.take()
    }

    /// Why the worker stopped, if it did.
    pub fn stopped_reason(&self) -> Option<String> {
        self.stopped
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Stop the screencast and end the worker.
    pub fn stop(&mut self) {
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            // The worker ends when the event channel closes, which happens when
            // the transport goes. Joining it here would wait for that, so the
            // thread is left to finish on its own.
            drop(worker);
        }
    }
}

impl Drop for Screencast {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screencast reports why it stopped, so the UI can say so.
    #[test]
    fn a_stopped_screencast_says_why() {
        let (tx, rx) = std::sync::mpsc::channel();
        let stopped: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let worker_stopped = stopped.clone();
        let worker = std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let Event::PageScreencastFrame(frame) = event else {
                    continue;
                };
                let _ = frame;
            }
            *worker_stopped.lock().unwrap() = Some("the browser connection closed".into());
        });
        drop(tx); // the channel closes, as it does when the browser goes
        worker.join().unwrap();
        assert_eq!(
            stopped.lock().unwrap().as_deref(),
            Some("the browser connection closed")
        );
    }
}
