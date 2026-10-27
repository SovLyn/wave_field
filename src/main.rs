mod app;
mod gpu;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    use wasm_bindgen::JsCast;
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        let result = async {
            let window = web_sys::window().ok_or("No browser window")?;
            let document = window.document().ok_or("No document")?;
            let canvas = document
                .get_element_by_id("wave-canvas")
                .ok_or("Canvas not found")?
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| "Invalid canvas")?;
            eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
                )
                .await
                .map_err(|_| {
                    "Could not initialize a WebGPU device. Check browser and GPU support."
                })?;
            if let Some(status) = document.get_element_by_id("status") {
                status.set_attribute("hidden", "").ok();
            }
            Ok::<(), &str>(())
        }
        .await;
        if let Err(message) = result {
            if let Some(status) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("status"))
            {
                status.set_text_content(Some(message));
                status.remove_attribute("hidden").ok();
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "Wave Field",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
