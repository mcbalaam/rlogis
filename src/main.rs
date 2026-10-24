mod app;
mod log;
mod platform;
mod radio;
mod theme;

use app::RlogisApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rlogis")
            .with_app_id("rlogis")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "rlogis",
        options,
        Box::new(|cc| Ok(Box::new(RlogisApp::new(cc)) as Box<dyn eframe::App>)),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast as _;

    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("rlogis_canvas")
            .expect("missing #rlogis_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#rlogis_canvas is not a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(RlogisApp::new(cc)) as Box<dyn eframe::App>)),
            )
            .await
            .expect("failed to start rlogis");
    });
}
