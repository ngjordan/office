//! The browser side of the suite: the eframe web runner, and the app the
//! runner owns.

use iridium_office::{brand, suite::Suite};

const CANVAS_ID: &str = "workspace_canvas";

/// Start the suite in the browser.
///
/// The page supplies the canvas; everything else is the same suite the
/// desktop app runs.
pub fn start() {
    let Some(canvas) = canvas() else {
        log::error!("no <canvas id=\"{CANVAS_ID}\"> on the page");
        return;
    };

    let mut web_options = eframe::WebOptions::default();
    // `?webgl` forces WebGL2 for a browser whose WebGPU adapter will not
    // start. The page reloads with it when the first attempt fails.
    if query_flag("webgl")
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(c) = &mut web_options.wgpu_options.wgpu_setup
    {
        c.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }

    let title = brand("APP_NAME").to_string();
    // On wasm the runner's start is a future the browser drives from its
    // event loop, so there is nothing to await here. The runner is created
    // inside the task: it has to outlive the start, and the future borrows
    // it.
    wasm_bindgen_futures::spawn_local(async move {
        let runner = eframe::WebRunner::new();
        let start = runner.start(
            canvas,
            web_options,
            Box::new(move |cc| {
                Ok(Box::new(App {
                    suite: Suite::new(Vec::new()),
                    _cc: cc.egui_ctx.clone(),
                }))
            }),
        );
        if let Err(e) = start.await {
            log::error!("the suite did not start: {e:?}");
            if let Some(el) = status_element() {
                el.set_text_content(Some(
                    "The suite did not start in this browser. Try Chrome, Edge or Firefox with WebGPU, or add ?webgl to the URL.",
                ));
            }
        }
    });
}

/// The suite's own state, owned by eframe.
struct App {
    suite: Suite,
    /// The egui context, kept so the app can ask for a repaint.
    _cc: egui::Context,
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
        self.suite.ui(ui, frame.storage());
        if self.suite.after_pass(frame) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self) {
        self.suite.save_state();
    }
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
