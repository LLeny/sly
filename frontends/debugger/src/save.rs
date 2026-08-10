use gpui::Context;

pub(crate) fn browse_for_file<V: 'static>(
    cx: &mut Context<V>,
    then: impl FnOnce(&std::path::Path, &mut V, &mut Context<V>) + 'static,
) {
    #[cfg(target_family = "wasm")]
    {
        let _ = (cx, then);
        crate::web_dnd::open_file_picker();
    }

    #[cfg(not(target_family = "wasm"))]
    {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });

        cx.spawn(async move |this: gpui::WeakEntity<V>, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };

            let _ = this.update(cx, |this, cx| then(&path, this, cx));
        })
        .detach();
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn write_file<V: 'static>(
    path: std::path::PathBuf,
    bytes: Vec<u8>,
    cx: &mut Context<V>,
    then: impl FnOnce(std::path::PathBuf, std::io::Result<()>, &mut V, &mut Context<V>) + 'static,
) {
    let background = cx.background_executor().clone();
    cx.spawn(async move |this: gpui::WeakEntity<V>, cx| {
        let (path, result) = background
            .spawn(async move {
                let result = std::fs::write(&path, bytes);
                (path, result)
            })
            .await;
        let _ = this.update(cx, |this, cx| then(path, result, this, cx));
    })
    .detach();
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn save_bin(name: String, bytes: Vec<u8>, cx: &mut gpui::App) {
    let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let rx = cx.prompt_for_new_path(&dir, Some(&name));
    let background = cx.background_executor().clone();

    cx.spawn(async move |_| {
        let path = match rx.await {
            Ok(Ok(Some(path))) => path,
            Ok(Err(err)) => {
                tracing::error!("save dialog failed: {err}");
                return;
            }
            _ => return,
        };

        background
            .spawn(async move {
                if let Err(err) = std::fs::write(&path, &bytes) {
                    tracing::error!("failed to write {}: {err}", path.display());
                }
            })
            .detach();
    })
    .detach();
}

#[cfg(target_family = "wasm")]
pub(crate) fn save_bin(name: String, bytes: Vec<u8>, _cx: &mut gpui::App) {
    if let Err(err) = self::download(&name, &bytes) {
        tracing::error!("download failed: {err:?}");
    }
}

#[cfg(target_family = "wasm")]
fn download(name: &str, bytes: &[u8]) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes).buffer());

    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;

    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("no document"))?;

    let anchor: web_sys::HtmlAnchorElement = document.create_element("a")?.unchecked_into();
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();

    web_sys::Url::revoke_object_url(&url)?;

    Ok(())
}
