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
//! - The CDP client is `headless_chrome`'s [`Transport`], used directly. Its
//!   `Browser` layer has a race in `new_tab` (it waits for a target event that
//!   may already have passed), and its launcher picks a port by asking the OS
//!   for a free one and hoping it stays free. We do neither.

use std::time::{Duration, Instant};

use headless_chrome::browser::transport::Transport;
use headless_chrome::protocol::cdp::Target;
use headless_chrome::protocol::cdp::Target::TargetInfo;
use headless_chrome::protocol::cdp::types::Event;
use headless_chrome::protocol::cdp::types::Method;

pub mod frame;
pub mod launch;
pub mod screencast;

use launch::{BrowserError, Launched};

/// A live browser, and the tab we drive.
pub struct Engine {
    /// The child process, the profile directory, and the port it listens on.
    ///
    /// `None` when we attached to a browser somebody else started.
    launched: Option<Launched>,
    /// The CDP connection to the browser endpoint.
    transport: std::sync::Arc<Transport>,
    /// The attached page.
    session: headless_chrome::browser::transport::SessionId,
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
        let (transport, launched) = match ws_url {
            Some(url) => {
                let transport = std::sync::Arc::new(
                    Transport::new(
                        url::Url::parse(url)
                            .map_err(|e| BrowserError::BadEndpoint(e.to_string()))?,
                        None,
                        Duration::from_secs(20),
                        None,
                    )
                    .map_err(|e| BrowserError::Cdp(format!("cannot reach the browser: {e}")))?,
                );
                (transport, None)
            }
            None => {
                let launched = launch::launch_in(profile)?;
                let transport = std::sync::Arc::new(
                    Transport::new(
                        url::Url::parse(&launched.ws_url)
                            .map_err(|e| BrowserError::BadEndpoint(e.to_string()))?,
                        None,
                        Duration::from_secs(20),
                        None,
                    )
                    .map_err(|e| BrowserError::Cdp(format!("cannot reach the browser: {e}")))?,
                );
                (transport, Some(launched))
            }
        };

        // A tab, attached. `Target.createTarget` then `Target.attachToTarget`
        // is the flow that works on every Chromium build we have met.
        let created = transport
            .call_method_on_browser(Target::CreateTarget {
                url: "about:blank".to_string(),
                left: None,
                top: None,
                width: None,
                height: None,
                window_state: None,
                browser_context_id: None,
                enable_begin_frame_control: None,
                new_window: None,
                background: None,
                for_tab: None,
                hidden: None,
            })
            .map_err(|e| BrowserError::Cdp(format!("Target.createTarget: {e}")))?;

        let attached = transport
            .call_method_on_browser(Target::AttachToTarget {
                target_id: created.target_id.clone(),
                // Not flat. `headless_chrome`'s transport routes messages to a
                // session with `Target.sendMessageToTarget`, which is the
                // non-flat protocol. Attaching flat makes the browser expect
                // a `sessionId` attribute on each message instead, and every
                // page-level call is rejected.
                flatten: None,
            })
            .map_err(|e| BrowserError::Cdp(format!("Target.attachToTarget: {e}")))?;

        Ok(Self {
            launched,
            transport,
            session: headless_chrome::browser::transport::SessionId::from(attached.session_id),
            target_id: created.target_id,
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
    pub fn call<C>(&self, method: C) -> Result<C::ReturnObject, BrowserError>
    where
        C: Method + serde::Serialize,
    {
        self.transport
            .call_method_on_target(self.session.clone(), method)
            .map_err(|e| BrowserError::Cdp(e.to_string()))
    }

    /// Listen for events on the attached page.
    pub fn events(&self) -> std::sync::mpsc::Receiver<Event> {
        self.transport.listen_to_target_events(self.session.clone())
    }

    /// Evaluate an expression in the page and return its value as a string.
    ///
    /// The workhorse for "what does this page actually say", used by the
    /// title bar and by the tests.
    pub fn evaluate(&self, expression: &str) -> Result<String, BrowserError> {
        let result = self
            .call(headless_chrome::protocol::cdp::Runtime::Evaluate {
                expression: expression.to_string(),
                object_group: None,
                include_command_line_api: None,
                silent: None,
                context_id: None,
                return_by_value: Some(true),
                generate_preview: None,
                user_gesture: None,
                await_promise: None,
                throw_on_side_effect: None,
                timeout: None,
                disable_breaks: None,
                repl_mode: None,
                allow_unsafe_eval_blocked_by_csp: None,
                unique_context_id: None,
                serialization_options: None,
            })
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        Ok(result
            .result
            .value
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default())
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
        // One event channel for the page. The screencast takes it over.
        let events = self.transport.listen_to_target_events(self.session.clone());
        let cast = screencast::Screencast::start(
            self.transport.clone(),
            self.session.clone(),
            events,
            format,
            max_width,
            max_height,
        )
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
        let _ = self.transport.call_method_on_target(
            self.session.clone(),
            headless_chrome::protocol::cdp::Page::StopScreencast(None),
        );
    }

    /// The `TargetInfo` of the attached page.
    pub fn target_info(&self) -> Result<TargetInfo, BrowserError> {
        let targets = self
            .transport
            .call_method_on_browser(Target::GetTargets { filter: None })
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        targets
            .target_infos
            .into_iter()
            .find(|t| t.target_id == self.target_id)
            .ok_or_else(|| BrowserError::Cdp("attached target vanished".to_string()))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Close the browser politely, then make sure it is gone. `Launched`'s
        // own Drop handles the second half; this only asks nicely first.
        let _ = self
            .transport
            .call_method_on_browser(headless_chrome::protocol::cdp::Browser::Close(None));
        self.transport.shutdown();
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
            .call(headless_chrome::protocol::cdp::Page::Navigate {
                url: "data:text/html,<body style='background:%23c00'>x</body>".to_string(),
                referrer: None,
                transition_Type: None,
                frame_id: None,
                referrer_policy: None,
            })
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
