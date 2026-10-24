//! Cross-platform "pick a log file" dialog.
//!
//! Native uses `rfd` on a worker thread so the UI never blocks. The Web build
//! drives a hidden `<input type="file">` directly, which also lets us detect the
//! browser's `cancel` event (rfd's wasm dialog can leave a cancelled pick
//! pending forever).

use std::sync::mpsc::Sender;

pub enum PickOutcome {
    Loaded { bytes: Vec<u8> },
    Cancelled,
    Failed(String),
}

/// Opens the system file picker. The result is delivered through `tx`.
pub fn pick_log_file(ctx: &egui::Context, tx: Sender<PickOutcome>) {
    imp::pick(ctx, tx);
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    pub fn pick(ctx: &egui::Context, tx: Sender<PickOutcome>) {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let selected = rfd::FileDialog::new()
                .set_title("Open game log")
                .add_filter("Game logs", &["log", "txt"])
                .add_filter("All files", &["*"])
                .pick_file();

            let outcome = match selected {
                Some(path) => match std::fs::read(&path) {
                    Ok(bytes) => PickOutcome::Loaded { bytes },
                    Err(error) => PickOutcome::Failed(format!("{}: {error}", path.display())),
                },
                None => PickOutcome::Cancelled,
            };

            let _ = tx.send(outcome);
            ctx.request_repaint();
        });
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{FileReader, HtmlInputElement};

    pub fn pick(ctx: &egui::Context, tx: Sender<PickOutcome>) {
        let fail = |message: &str| {
            let _ = tx.send(PickOutcome::Failed(message.to_owned()));
            ctx.request_repaint();
        };

        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            fail("browser document is unavailable");
            return;
        };
        let Some(body) = document.body() else {
            fail("browser document has no body");
            return;
        };
        let input = match document.create_element("input") {
            Ok(element) => match element.dyn_into::<HtmlInputElement>() {
                Ok(input) => input,
                Err(_) => {
                    fail("could not create a file input");
                    return;
                }
            },
            Err(_) => {
                fail("could not create a file input");
                return;
            }
        };

        input.set_type("file");
        input.set_accept(".log,.txt,text/plain");
        let _ = input.set_attribute("style", "display: none");
        if body.append_child(&input).is_err() {
            fail("could not attach the file input");
            return;
        }

        let input_for_change = input.clone();
        let ctx_for_change = ctx.clone();
        let tx_for_change = tx.clone();
        let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
            let Some(file) = input_for_change.files().and_then(|files| files.get(0)) else {
                input_for_change.remove();
                return;
            };

            let reader = match FileReader::new() {
                Ok(reader) => reader,
                Err(_) => {
                    let _ = tx_for_change.send(PickOutcome::Failed(
                        "could not create a FileReader".to_owned(),
                    ));
                    ctx_for_change.request_repaint();
                    input_for_change.remove();
                    return;
                }
            };

            let reader_for_load = reader.clone();
            let ctx_for_load = ctx_for_change.clone();
            let tx_for_load = tx_for_change.clone();
            let on_load = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
                let outcome = match reader_for_load.result() {
                    Ok(value) => match value.as_string() {
                        Some(text) => PickOutcome::Loaded {
                            bytes: text.into_bytes(),
                        },
                        None => PickOutcome::Failed("the file is not valid UTF-8 text".to_owned()),
                    },
                    Err(_) => PickOutcome::Failed("could not read the file".to_owned()),
                };
                let _ = tx_for_load.send(outcome);
                ctx_for_load.request_repaint();
            });
            reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
            on_load.forget();

            let ctx_for_error = ctx_for_change.clone();
            let tx_for_error = tx_for_change.clone();
            let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
                let _ = tx_for_error.send(PickOutcome::Failed(
                    "the browser could not read the file".to_owned(),
                ));
                ctx_for_error.request_repaint();
            });
            reader.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            on_error.forget();

            if reader.read_as_text(&file).is_err() {
                let _ = tx_for_change.send(PickOutcome::Failed(
                    "the browser could not read the file".to_owned(),
                ));
                ctx_for_change.request_repaint();
            }

            input_for_change.remove();
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();

        let input_for_cancel = input.clone();
        let ctx_for_cancel = ctx.clone();
        let on_cancel = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
            let _ = tx.send(PickOutcome::Cancelled);
            ctx_for_cancel.request_repaint();
            input_for_cancel.remove();
        });
        input.set_oncancel(Some(on_cancel.as_ref().unchecked_ref()));
        on_cancel.forget();

        // Must stay inside the click handler for browsers that require a
        // transient user activation to open the picker.
        input.click();
    }
}
