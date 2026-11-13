//! The embedded browser: find an installed Chromium, launch it under our own
//! profile, and talk to it over the DevTools Protocol.
//!
//! The engine is the user's own browser (Brave first, then Edge, then Chrome).
//! Nothing is bundled: no Chromium zip to sign, notarize, or security-update.
//!
//! Split in two, deliberately:
//!
//! - [`launch`] owns the child process. It picks the executable, writes a
//!   profile we own, starts the browser with a debugging port, waits for it to
//!   be ready, and kills it again on the way out.
//! - [`cdp`] speaks the protocol: JSON over a WebSocket, one reader thread.
//!   We drive it ourselves rather than through a library, because the one
//!   that was here generated its protocol code with a GPL tool.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub mod cdp;
pub mod frame;
pub mod launch;
pub mod screencast;

use cdp::Cdp;
use launch::{BrowserError, Launched};

/// A live browser, and the tab we drive.
pub struct Engine {
    /// The child process, the profile directory, and the port it listens on.
    ///
    /// `None` when we attached to a browser somebody else started.
    launched: Option<Launched>,
    /// The CDP connection to the browser endpoint.
    cdp: Arc<Cdp>,
    /// The page's target id.
    target_id: String,
    /// The frames coming from the browser, if the screencast is running.
    screencast: Option<screencast::Screencast>,
}

impl Engine {
    /// Find a browser, start it, and open one tab on `about:blank`.
    pub fn start() -> Result<Self, BrowserError> {
        Self::start_with(None)
    }

    /// Start the browser, or attach to an already-running one at `ws_url`.
    ///
    /// The attach path exists for tests and for a browser someone started
    /// deliberately. It is never used against the user's everyday browser:
    /// that profile holds their sessions, and Chrome 136+ refuses remote
    /// debugging on the default profile anyway.
    pub fn start_with(ws_url: Option<&str>) -> Result<Self, BrowserError> {
        Self::start_full(ws_url, None)
    }

    /// Start the engine on a profile of its own.
    ///
    /// `profile` is for a second engine that must not share the suite's
    /// profile; Chromium will not start twice on one profile.
    pub fn start_in(profile: Option<std::path::PathBuf>) -> Result<Self, BrowserError> {
        Self::start_full(None, profile)
    }

    fn start_full(
        ws_url: Option<&str>,
        profile: Option<std::path::PathBuf>,
    ) -> Result<Self, BrowserError> {
        let (cdp, launched) = match ws_url {
            Some(url) => (
                Arc::new(Cdp::connect(url).map_err(BrowserError::Cdp)?),
                None,
            ),
            None => {
                let launched = launch::launch_in(profile)?;
                let cdp = Cdp::connect(&launched.ws_url).map_err(BrowserError::Cdp)?;
                (Arc::new(cdp), Some(launched))
            }
        };

        // A tab, attached. `Target.createTarget` then `Target.attachToTarget`
        // is the flow that works on every Chromium build we have met.
        let target_id = cdp.attach().map_err(BrowserError::Cdp)?;

        Ok(Self {
            launched,
            cdp,
            target_id,
            screencast: None,
        })
    }

    /// The browser's product string, e.g. "Chrome/148.0.7778.96".
    ///
    /// Read from `/json/version` at launch, so it costs nothing here.
    pub fn product(&self) -> String {
        self.launched
            .as_ref()
            .map(|l| l.product().to_string())
            .unwrap_or_default()
    }

    /// Which browser this is.
    pub fn browser(&self) -> Option<String> {
        self.launched
            .as_ref()
            .map(|l| l.browser().label().to_string())
    }

    /// The port the browser's debugging endpoint is on.
    pub fn port(&self) -> Option<u16> {
        self.launched.as_ref().map(|l| l.port())
    }

    /// Call a CDP method on the attached page.
    ///
    /// `method` is a DevTools method name ("Page.navigate"), `params` its
    /// arguments. The answer is the method's `result` object.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, BrowserError> {
        self.cdp
            .call_page(method, params)
            .map_err(BrowserError::Cdp)
    }

    /// Evaluate an expression in the page and return its value as a string.
    ///
    /// The workhorse for "what does this page actually say", used by the
    /// title bar and by the tests.
    pub fn evaluate(&self, expression: &str) -> Result<String, BrowserError> {
        let result = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
        )?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// The page's target id.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Start sending the page as frames.
    ///
    /// The frames are decoded on a worker thread, so the UI thread is never
    /// held up by a page that is changing fast.
    pub fn start_screencast(
        &mut self,
        format: frame::FrameFormat,
        max_width: u32,
        max_height: u32,
    ) -> Result<(), BrowserError> {
        let events = self.cdp.events();
        let cast =
            screencast::Screencast::start(self.cdp.clone(), events, format, max_width, max_height)
                .map_err(BrowserError::Cdp)?;
        self.screencast = Some(cast);
        Ok(())
    }

    /// The newest frame from the page, if one has arrived.
    pub fn take_frame(&self) -> Option<frame::Frame> {
        self.screencast.as_ref()?.take_frame()
    }

    /// Whether frames are arriving.
    pub fn has_frame(&self) -> bool {
        self.screencast
            .as_ref()
            .map(|c| c.frames.has_frame())
            .unwrap_or(false)
    }

    /// Stop sending frames.
    ///
    /// A browser that is not on screen should not be painting frames.
    pub fn stop_screencast(&mut self) {
        if let Some(mut cast) = self.screencast.take() {
            cast.stop();
        }
        let _ = self.cdp.call_page("Page.stopScreencast", json!({}));
    }

    /// The target id of the attached page.
    pub fn target_info(&self) -> Result<String, BrowserError> {
        Ok(self.target_id.clone())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Close the browser politely, then make sure it is gone. `Launched`'s
        // own Drop handles the second half; this only asks nicely first.
        let _ = self.cdp.call_browser("Browser.close", json!({}));
    }
}

/// How long the browser took to come up, for the startup log.
pub fn elapsed_since(start: Instant) -> Duration {
    start.elapsed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real browser produces a real frame that decodes to real pixels.
    ///
    /// This is the whole frame pipeline: the browser paints, the screencast
    /// carries it, the worker decodes it, and the slot holds it.
    #[test]
    fn a_real_browser_produces_a_frame() {
        if crate::browser::launch::find_browser().is_none() {
            eprintln!("no Chromium-family browser installed; skipping");
            return;
        }
        let profile = crate::browser::launch::private_profile_dir(&format!(
            "frame-test-{}",
            std::process::id()
        ))
        .expect("profile");
        let mut engine = Engine::start_in(Some(profile)).expect("engine starts");

        // A page with something on it, so the frame is not blank.
        engine
            .call(
                "Page.navigate",
                json!({ "url": "data:text/html,<body style='background:%23c00'>x</body>" }),
            )
            .expect("navigate");
        std::thread::sleep(std::time::Duration::from_secs(2));

        engine
            .start_screencast(super::frame::FrameFormat::Jpeg, 640, 480)
            .expect("the screencast starts");

        // Wait for a frame to arrive and be decoded.
        let mut frame = None;
        for _ in 0..100 {
            if let Some(f) = engine.take_frame() {
                frame = Some(f);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let frame = frame.expect("a frame arrives");
        assert!(frame.width > 0 && frame.height > 0, "the frame has a size");
        assert_eq!(
            frame.pixels.len(),
            (frame.width * frame.height * 4) as usize,
            "RGBA, four bytes a pixel"
        );
        engine.stop_screencast();
    }

    /// The engine starts, opens a tab, and shuts down. Skipped when no
    /// browser is installed, which is a supported state, not a failure.
    #[test]
    fn engine_starts_and_closes() {
        if crate::browser::launch::find_browser().is_none() {
            eprintln!("no Chromium-family browser installed; skipping");
            return;
        }
        // Its own profile: another test may be running a browser at the same
        // time, and Chromium will not start twice on one profile.
        let profile = crate::browser::launch::private_profile_dir(&format!(
            "engine-test-{}",
            std::process::id()
        ))
        .expect("profile");
        let engine = Engine::start_in(Some(profile)).expect("engine starts");
        assert!(!engine.target_id().is_empty());
        // A page-level call must answer on a live tab.
        let title = engine.evaluate("document.title").expect("evaluate answers");
        assert!(
            title.is_empty() || title.len() < 200,
            "a page title, not a page of text"
        );
        drop(engine);
    }
}
