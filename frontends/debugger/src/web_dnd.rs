use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use crate::root::Root;

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

    let on_drop =
        Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |event: web_sys::DragEvent| {
            event.prevent_default();
            let Some(file) = event
                .data_transfer()
                .and_then(|dt| dt.files())
                .and_then(|files| files.get(0))
            else {
                return;
            };

            let name = file.name();
            let mut async_cx = async_cx.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let Ok(buffer) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                else {
                    tracing::error!("web drag-and-drop: failed to read dropped file");
                    return;
                };

                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                let _ = handle.update(&mut async_cx, |root, _window, cx| {
                    root.load_rom(name, bytes, cx);
                });
            });
        });

    let _ = target.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref());
    on_drop.forget();
}
