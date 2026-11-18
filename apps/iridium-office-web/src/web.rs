//! The browser side of the suite: the eframe web runner, and the app the
//! runner owns.

use iridium_office::suite::Suite;

const CANVAS_ID: &str = "workspace_canvas";

/// Start the suite in the browser.
///
/// The page supplies the canvas; everything else is the same suite the
/// desktop app runs.
pub fn start() {
    // Two entries reach here: the wasm-bindgen start hook, which runs when
    // the module is instantiated, and the page, which calls the exported
    // entry point. A second runner would be a second GPU device drawing to
    // the same canvas, so the first one wins.
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        log::warn!("the suite is already running; not starting it twice");
        return;
    }

    // Without a logger every failure in the runner is invisible: the console
    // stays empty and the page just never draws.
    let _ = eframe::WebLogger::init(log::LevelFilter::Debug);

    let mut web_options = eframe::WebOptions::default();
    // `?webgl` forces WebGL2 for a browser whose WebGPU adapter will not
    // start. The page reloads with it when the first attempt fails.
    if query_flag("webgl")
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(c) = &mut web_options.wgpu_options.wgpu_setup
    {
        c.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }

    // The window title comes from the page (the <title> element), so the
    // brand name is not needed here.
    // On wasm the runner's start is a future the browser drives from its
    // event loop, so there is nothing to await here. The runner is created
    // inside the task: it has to outlive the start, and the future borrows
    // it.
    wasm_bindgen_futures::spawn_local(async move {
        // The page's canvas may not exist yet: Trunk puts the module script
        // in the head, so the module is instantiated before the body has
        // been parsed. Waiting for the DOM is what makes the lookup work.
        wait_for_dom().await;
        let Some(canvas) = canvas() else {
            log::error!("no <canvas id=\"{CANVAS_ID}\"> on the page");
            if let Some(el) = status_element() {
                el.set_text_content(Some("The page has no canvas for the suite to draw on."));
            }
            return;
        };

        // WebGPU where the browser has a GPU, WebGL2 where it does not.
        //
        // The retry is not only for a browser without WebGPU at all: a
        // browser can have it and still fail to get an adapter (Linux
        // Chrome without Vulkan), and the app must start either way.
        let runner = eframe::WebRunner::new();
        let start = runner.start(canvas.clone(), web_options, Box::new(make_app));
        if let Err(e) = start.await {
            log::warn!("WebGPU did not start ({e:?}); trying WebGL2");
            // A canvas holds one context type for ever, and the failed
            // attempt has already taken this one, so the fallback draws on
            // a new element.
            let Some(canvas) = fresh_canvas() else {
                log::error!("no canvas to fall back to");
                return;
            };
            let mut options = eframe::WebOptions::default();
            if let eframe::egui_wgpu::WgpuSetup::CreateNew(c) = &mut options.wgpu_options.wgpu_setup {
                c.instance_descriptor.backends = eframe::wgpu::Backends::GL;
            }
            let runner = eframe::WebRunner::new();
            if let Err(e) = runner.start(canvas, options, Box::new(make_app)).await {
                log::error!("the suite did not start: {e:?}");
                if let Some(el) = status_element() {
                    el.set_text_content(Some(
                        "The suite did not start in this browser. It needs WebGPU or WebGL2.",
                    ));
                }
            }
        }
    });
}

/// The app the runner owns.
///
/// A function, not a closure, so the WebGL2 fallback can have its own.
fn make_app(
    cc: &eframe::CreationContext<'_>,
) -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(Box::new(App {
        suite: Suite::new(Vec::new()),
        _cc: cc.egui_ctx.clone(),
        drawn: false,
    }))
}

/// A new canvas to draw on, in place of the one the failed attempt took.
fn fresh_canvas() -> Option<web_sys::HtmlCanvasElement> {
    use wasm_bindgen::JsCast as _;
    let old = canvas()?;
    // A shallow copy: the canvas has no children, and the attributes (the
    // id, the style) are what the app needs.
    let new: web_sys::HtmlCanvasElement = old.clone_node().ok()?.dyn_into().ok()?;
    old.parent_node()?.replace_child(&new, &old).ok()?;
    Some(new)
}

/// The suite's own state, owned by eframe.
struct App {
    suite: Suite,
    /// The egui context, kept so the app can ask for a repaint.
    _cc: egui::Context,
    /// Whether the suite has drawn yet, so the page's loading message goes
    /// down on the first frame.
    drawn: bool,
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // eframe's autosave, not the exit path: `save_state` is the exit
        // path, and calling it here made the suite quit by itself.
        self.suite.save(storage);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.suite.logic(ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.suite.raw_input_hook(ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // The web runner calls `App::ui` and never `App::logic`, so the apps'
        // own logic (their fonts, their state) has to run here. The native
        // runner calls both, in this order.
        self.suite.logic(ui.ctx());
        self.suite.ui(ui, frame.storage());
        if self.suite.after_pass(frame) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // The page shows a message until the suite draws. Take it down on the
        // first frame, so a working app is not covered by "Loading...".
        if !self.drawn {
            self.drawn = true;
            if let Some(el) = status_element() {
                el.set_attribute("hidden", "");
            }
        }
    }

    fn on_exit(&mut self) {
        self.suite.save_state();
    }
}

/// Wait until the page's body is parsed.
///
/// Trunk puts the module script in the head, so the module is instantiated
/// while the body is still empty and the canvas is not there yet.
async fn wait_for_dom() {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    if document.ready_state() != "loading" {
        return;
    }
    // The window's load event fires when the document has been parsed.
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = window.set_onload(Some(&resolve));
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    let _ = window.set_onload(None);
}

/// The canvas the page made for us.
fn canvas() -> Option<web_sys::HtmlCanvasElement> {
    use wasm_bindgen::JsCast;
    let doc = web_sys::window()?.document()?;
    doc.get_element_by_id(CANVAS_ID)?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .ok()
}

/// The element the page uses to show a message while the suite loads.
fn status_element() -> Option<web_sys::Element> {
    let doc = web_sys::window()?.document()?;
    doc.get_element_by_id("workspace_status")
}

/// Is `flag` in the URL's query string?
fn query_flag(flag: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(search) = window.location().search() else {
        return false;
    };
    search
        .trim_start_matches('?')
        .split('&')
        .any(|kv| kv.split('=').next() == Some(flag))
}
