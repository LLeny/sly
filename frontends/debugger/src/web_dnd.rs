use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use crate::root::Root;

const FILE_INPUT_ID: &str = "sly-rom-file-input";

pub(crate) fn install(handle: gpui::WindowHandle<Root>, cx: &gpui::App) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        tracing::error!("web drag-and-drop: no document available");
        return;
    };
    let target: &web_sys::EventTarget = document.as_ref();
    let async_cx = cx.to_async();

    let dragover = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|event: web_sys::DragEvent| {
        event.prevent_default();
    });
    let _ = target.add_event_listener_with_callback("dragover", dragover.as_ref().unchecked_ref());
    dragover.forget();

    let on_drop = {
        let async_cx = async_cx.clone();
        Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |event: web_sys::DragEvent| {
            event.prevent_default();
            let Some(file) = event
                .data_transfer()
                .and_then(|dt| dt.files())
                .and_then(|files| files.get(0))
            else {
                return;
            };
            load_file(handle, async_cx.clone(), file);
        })
    };
    let _ = target.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref());
    on_drop.forget();

    install_file_input(handle, &document, async_cx);
}

fn install_file_input(
    handle: gpui::WindowHandle<Root>,
    document: &web_sys::Document,
    async_cx: gpui::AsyncApp,
) {
    let Some(body) = document.body() else {
        tracing::error!("web file picker: no document body available");
        return;
    };
    let input: web_sys::HtmlInputElement = match document.create_element("input") {
        Ok(element) => element.unchecked_into(),
        Err(_) => {
            tracing::error!("web file picker: failed to create input element");
            return;
        }
    };
    input.set_type("file");
    input.set_id(FILE_INPUT_ID);
    let _ = input.style().set_property("display", "none");

    let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(input) = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };
        input.set_value("");
        load_file(handle, async_cx.clone(), file);
    });
    let _ = input.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
    on_change.forget();

    let _ = body.append_child(&input);
}

pub(crate) fn open_file_picker() {
    let Some(input) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(FILE_INPUT_ID))
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        tracing::error!("web file picker: input element not installed");
        return;
    };
    input.click();
}

fn load_file(handle: gpui::WindowHandle<Root>, mut async_cx: gpui::AsyncApp, file: web_sys::File) {
    let name = file.name();
    wasm_bindgen_futures::spawn_local(async move {
        let Ok(buffer) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await else {
            tracing::error!("web file picker: failed to read file");
            return;
        };

        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        let _ = handle.update(&mut async_cx, |root, _window, cx| {
            root.load_dropped(name, bytes, cx);
        });
    });
}
