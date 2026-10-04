use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use toml::{Table, Value};

pub const ACCOUNT_URL: &str = "https://art.layle.dev/account";

#[cfg(target_family = "wasm")]
const STORAGE_KEY: &str = "sly.settings.toml";

struct State {
    path: PathBuf,
    document: Value,
    baselines: HashMap<String, Value>,
    error: Option<String>,
}

impl State {
    fn load(path: PathBuf) -> Self {
        let (document, error) = match read(&path) {
            Ok(document) => (document, None),
            Err(error) => (Table::new().into(), Some(error)),
        };

        Self {
            path,
            document,
            baselines: HashMap::new(),
            error,
        }
    }
}

static STATE: LazyLock<Mutex<Option<State>>> = LazyLock::new(|| Mutex::new(None));

pub fn default_path() -> PathBuf {
    #[cfg(not(target_family = "wasm"))]
    return storage::FsStore::new().root().join("settings.toml");

    #[cfg(target_family = "wasm")]
    PathBuf::from(STORAGE_KEY)
}

pub fn initialize(path: Option<&Path>) -> Result<(), String> {
    let path = path.map(Path::to_path_buf).unwrap_or_else(default_path);
    *STATE.lock().unwrap() = Some(State::load(path));

    with_state(|_| Ok(()))
}

/// Reloads only if the path changed.
pub fn select_path(path: &Path) -> Result<(), String> {
    {
        let mut guard = STATE.lock().unwrap();

        if guard.as_ref().is_none_or(|s| s.path != path) {
            *guard = Some(State::load(path.to_owned()));
        }
    }

    with_state(|_| Ok(()))
}

pub fn normalize_key(key: &str) -> Result<String, String> {
    let key = key.trim();

    if key.bytes().any(|b| !(32..=126).contains(&b)) {
        return Err("API key must contain printable ASCII characters only".into());
    }

    Ok(key.to_owned())
}

pub fn api_key() -> Result<String, String> {
    with_state(|s| {
        normalize_key(
            s.document
                .get("artwork")
                .and_then(|a| a.get("api_key"))
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
    })
}

pub fn authorization() -> Option<String> {
    api_key()
        .ok()
        .filter(|key| !key.is_empty())
        .map(|key| format!("Bearer {key}"))
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    let key = Value::String(normalize_key(key)?);
    let edit = |v: &mut Value| {
        child(v, "artwork")?
            .as_table_mut()
            .ok_or("Invalid artwork section")?
            .insert("api_key".into(), key.clone());

        Ok(())
    };

    with_state(|s| {
        transaction(&s.path, edit)?;
        edit(&mut s.document)
    })
}

pub fn frontend<T: DeserializeOwned + Serialize + Default>(name: &str) -> Result<T, String> {
    with_state(|s| {
        let settings: T = match s.document.get("frontends").and_then(|f| f.get(name)) {
            Some(v) => v
                .clone()
                .try_into()
                .map_err(|_| format!("Invalid {name} settings"))?,
            None => T::default(),
        };

        // Defaults go in the baseline too, in case another app saves a field that was missing.
        let baseline = Value::try_from(&settings).map_err(|_| "Cannot encode frontend settings")?;
        s.baselines.insert(name.into(), baseline);

        Ok(settings)
    })
}

pub fn save_frontend<T: Serialize>(name: &str, settings: &T) -> Result<(), String> {
    let after = Value::try_from(settings).map_err(|_| "Cannot encode frontend settings")?;

    with_state(|s| {
        let mut before = s.baselines.get(name).cloned().or_else(|| {
            s.document
                .get("frontends")
                .and_then(|f| f.get(name))
                .cloned()
        });

        // Native clients without a schema only send the fields they know about.
        if !s.baselines.contains_key(name)
            && let (Some(old), Some(new)) = (
                before.as_mut().and_then(Value::as_table_mut),
                after.as_table(),
            )
        {
            old.retain(|key, _| new.contains_key(key));
        }

        let edit = |v: &mut Value| {
            merge(
                child(child(v, "frontends")?, name)?,
                before.as_ref(),
                &after,
            );
            Ok(())
        };

        transaction(&s.path, edit)?;
        edit(&mut s.document)?;
        s.baselines.insert(name.into(), after);

        Ok(())
    })
}

pub fn frontend_json(name: &str) -> Result<String, String> {
    with_state(|s| {
        serde_json::to_string(
            &s.document
                .get("frontends")
                .and_then(|f| f.get(name))
                .cloned()
                .unwrap_or_else(|| Table::new().into()),
        )
        .map_err(|_| "Cannot encode settings".into())
    })
}

pub fn save_frontend_json(name: &str, json: &str) -> Result<(), String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| "Invalid settings JSON")?;

    if !v.is_object() {
        return Err("Frontend settings must be an object".into());
    }

    save_frontend(name, &v)
}

#[cfg(not(target_family = "wasm"))]
pub async fn set_api_key_async(key: String) -> Result<(), String> {
    set_api_key(&key)
}

#[cfg(not(target_family = "wasm"))]
pub async fn save_frontend_async<T: Serialize + 'static>(
    name: &'static str,
    settings: T,
) -> Result<(), String> {
    save_frontend(name, &settings)
}

#[cfg(target_family = "wasm")]
pub async fn set_api_key_async(key: String) -> Result<(), String> {
    browser_lock::run(move || set_api_key(&key)).await
}

#[cfg(target_family = "wasm")]
pub async fn save_frontend_async<T: Serialize + 'static>(
    name: &'static str,
    settings: T,
) -> Result<(), String> {
    browser_lock::run(move || save_frontend(name, &settings)).await
}

fn with_state<T>(f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
    let mut guard = STATE.lock().unwrap();
    let state = guard.get_or_insert_with(|| State::load(default_path()));
    if let Some(error) = &state.error {
        return Err(error.clone());
    }

    f(state)
}

fn parse(text: &str) -> Result<Value, String> {
    let v: Value = toml::from_str(text)
        .map_err(|_| "Invalid settings TOML; original file preserved".to_string())?;

    if !v.is_table() {
        return Err("Settings must be a table".into());
    }

    for key in ["artwork", "frontends"] {
        if v.get(key).is_some_and(|v| !v.is_table()) {
            return Err(format!("{key} must be a table"));
        }
    }

    if let Some(key) = v.get("artwork").and_then(|v| v.get("api_key")) {
        normalize_key(key.as_str().ok_or("artwork.api_key must be a string")?)?;
    }

    Ok(v)
}

fn child<'a>(v: &'a mut Value, key: &str) -> Result<&'a mut Value, String> {
    Ok(v.as_table_mut()
        .ok_or("Settings section must be a table")?
        .entry(key.to_owned())
        .or_insert_with(|| Table::new().into()))
}

fn merge(latest: &mut Value, before: Option<&Value>, after: &Value) {
    if before == Some(after) {
        return;
    }

    let (Some(target), Some(source)) = (latest.as_table_mut(), after.as_table()) else {
        *latest = after.clone();
        return;
    };

    for (key, value) in source {
        let old = before.and_then(|v| v.get(key));

        if old != Some(value) {
            merge(
                target
                    .entry(key.clone())
                    .or_insert_with(|| Table::new().into()),
                old,
                value,
            );
        }
    }

    if let Some(old) = before.and_then(Value::as_table) {
        for key in old.keys().filter(|key| !source.contains_key(*key)) {
            target.remove(key);
        }
    }
}

fn transaction(
    path: &Path,
    edit: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    #[cfg(not(target_family = "wasm"))]
    let _lock = {
        std::fs::create_dir_all(parent(path)).map_err(|e| e.to_string())?;

        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("toml.lock"))
            .map_err(|e| e.to_string())?;

        file.lock().map_err(|e| e.to_string())?;
        file
    };

    let mut latest = read(path)?;
    edit(&mut latest)?;

    let text = toml::to_string_pretty(&latest).map_err(|_| "Cannot encode settings")?;
    write(path, &text)
}

#[cfg(not(target_family = "wasm"))]
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

#[cfg(not(target_family = "wasm"))]
fn read(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Table::new().into()),
        Err(e) => Err(format!("Cannot read settings at {}: {e}", path.display())),
    }
}

#[cfg(not(target_family = "wasm"))]
fn write(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;

    let mut temporary = tempfile::NamedTempFile::new_in(parent(path)).map_err(|e| e.to_string())?;

    temporary
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.error.to_string())?;

    Ok(())
}

#[cfg(target_family = "wasm")]
fn browser_storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .ok_or_else(|| "Browser settings storage unavailable".into())
}

#[cfg(target_family = "wasm")]
fn read(_: &Path) -> Result<Value, String> {
    browser_storage()?
        .get_item(STORAGE_KEY)
        .map_err(|_| "Cannot read browser settings")?
        .map_or_else(|| Ok(Table::new().into()), |text| parse(&text))
}

#[cfg(target_family = "wasm")]
fn write(_: &Path, text: &str) -> Result<(), String> {
    browser_storage()?
        .set_item(STORAGE_KEY, text)
        .map_err(|_| "Cannot save browser settings".into())
}

#[cfg(target_family = "wasm")]
mod browser_lock {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "
        export async function slySettingsLock(callback) {
            if (!navigator.locks) throw new Error('Settings require browser Web Locks');
            return navigator.locks.request('sly.settings.toml', () => callback());
        }
    ")]
    extern "C" {
        #[wasm_bindgen(catch, js_name = slySettingsLock)]
        async fn lock(callback: &js_sys::Function) -> Result<JsValue, JsValue>;
    }

    pub async fn run(edit: impl FnOnce() -> Result<(), String> + 'static) -> Result<(), String> {
        let result = Rc::new(RefCell::new(None));
        let output = result.clone();
        let callback = Closure::once(move || {
            *output.borrow_mut() = Some(edit());
        });

        lock(callback.as_ref().unchecked_ref())
            .await
            .map_err(|_| "Cannot acquire browser settings lock")?;

        result
            .borrow_mut()
            .take()
            .ok_or("Browser settings update did not run")?
    }
}
