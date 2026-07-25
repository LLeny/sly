use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::{Entry, GameId, SaveStore, Slot, SlotInfo, StoreError, entry_path, game_prefixes};

const DB_NAME: &str = "sly";
const DB_VERSION: u32 = 1;
const STORE_NAME: &str = "saves";

#[derive(Clone, Default)]
pub struct IdbStore {
    mirror: Rc<RefCell<BTreeMap<String, Rc<[u8]>>>>,
    db: Rc<RefCell<Option<web_sys::IdbDatabase>>>,
}

impl IdbStore {
    pub fn new() -> Self {
        Self::default()
    }

    async fn load(&self, game: &GameId) -> Result<(), StoreError> {
        let db = self.connection().await?;
        let prefixes = game_prefixes(game);

        for prefix in prefixes {
            let entries = read_prefix(&db, &prefix).await?;
            let mut mirror = self.mirror.borrow_mut();
            for (key, value) in entries {
                mirror.entry(key).or_insert_with(|| value.into());
            }
        }

        Ok(())
    }

    async fn connection(&self) -> Result<web_sys::IdbDatabase, StoreError> {
        if let Some(db) = self.db.borrow().as_ref() {
            return Ok(db.clone());
        }

        let db = open_db().await?;
        *self.db.borrow_mut() = Some(db.clone());
        Ok(db)
    }

    /// `None` deletes. Writes are queued rather than awaited so a save does not
    /// block a frame; durability lands shortly after.
    fn queue(&self, key: String, value: Option<Rc<[u8]>>) {
        let store = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let db = match store.connection().await {
                Ok(db) => db,
                Err(err) => return tracing::error!("indexeddb open failed: {err}"),
            };

            let result = match &value {
                Some(bytes) => put(&db, &key, bytes).await,
                None => delete(&db, &key).await,
            };
            if let Err(err) = result {
                tracing::error!("indexeddb write {key} failed: {err}");
            }
        });
    }
}

impl SaveStore for IdbStore {
    fn read(&self, game: &GameId, entry: Entry) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .mirror
            .borrow()
            .get(&entry_path(game, entry))
            .map(|bytes| bytes.to_vec()))
    }

    fn write(&self, game: &GameId, entry: Entry, bytes: &[u8]) -> Result<(), StoreError> {
        let key = entry_path(game, entry);
        let shared: Rc<[u8]> = Rc::from(bytes);
        self.mirror.borrow_mut().insert(key.clone(), shared.clone());
        self.queue(key, Some(shared));
        Ok(())
    }

    fn remove(&self, game: &GameId, entry: Entry) -> Result<(), StoreError> {
        let key = entry_path(game, entry);
        self.mirror.borrow_mut().remove(&key);
        self.queue(key, None);
        Ok(())
    }

    fn slots(&self, game: &GameId) -> Result<Vec<SlotInfo>, StoreError> {
        let mirror = self.mirror.borrow();
        Ok(Slot::ALL
            .into_iter()
            .filter_map(|slot| {
                let key = entry_path(game, Entry::State(slot));
                mirror.get(&key).map(|bytes| SlotInfo {
                    slot,
                    len: bytes.len() as u64,
                })
            })
            .collect())
    }

    fn hydrate(
        &self,
        game: &GameId,
    ) -> impl std::future::Future<Output = Result<(), StoreError>> + 'static {
        let store = self.clone();
        let game = game.clone();
        async move { store.load(&game).await }
    }
}

fn js_err(context: &str, value: JsValue) -> StoreError {
    StoreError::Unavailable(format!("{context}: {value:?}"))
}

async fn open_db() -> Result<web_sys::IdbDatabase, StoreError> {
    let factory = web_sys::window()
        .and_then(|window| window.indexed_db().ok().flatten())
        .ok_or_else(|| StoreError::Unavailable("IndexedDB is not available".into()))?;

    let request = factory
        .open_with_u32(DB_NAME, DB_VERSION)
        .map_err(|err| js_err("opening database", err))?;

    let upgrade = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(request) = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::IdbOpenDbRequest>().ok())
        else {
            return;
        };

        let Some(db) = request
            .result()
            .ok()
            .and_then(|value| value.dyn_into::<web_sys::IdbDatabase>().ok())
        else {
            return;
        };

        if !db.object_store_names().contains(STORE_NAME) {
            let _ = db.create_object_store(STORE_NAME);
        }
    });
    request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    upgrade.forget();

    let value = await_request(request.clone().unchecked_into()).await?;
    value
        .dyn_into::<web_sys::IdbDatabase>()
        .map_err(|err| js_err("database handle", err))
}

fn store_for(
    db: &web_sys::IdbDatabase,
    mode: web_sys::IdbTransactionMode,
) -> Result<web_sys::IdbObjectStore, StoreError> {
    db.transaction_with_str_and_mode(STORE_NAME, mode)
        .map_err(|err| js_err("starting transaction", err))?
        .object_store(STORE_NAME)
        .map_err(|err| js_err("opening object store", err))
}

async fn put(db: &web_sys::IdbDatabase, key: &str, value: &[u8]) -> Result<(), StoreError> {
    let store = store_for(db, web_sys::IdbTransactionMode::Readwrite)?;
    let array = js_sys::Uint8Array::from(value);
    let request = store
        .put_with_key(&array, &JsValue::from_str(key))
        .map_err(|err| js_err("put", err))?;
    await_request(request).await.map(|_| ())
}

async fn delete(db: &web_sys::IdbDatabase, key: &str) -> Result<(), StoreError> {
    let store = store_for(db, web_sys::IdbTransactionMode::Readwrite)?;
    let request = store
        .delete(&JsValue::from_str(key))
        .map_err(|err| js_err("delete", err))?;
    await_request(request).await.map(|_| ())
}

async fn read_prefix(
    db: &web_sys::IdbDatabase,
    prefix: &str,
) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
    let store = store_for(db, web_sys::IdbTransactionMode::Readonly)?;

    let upper = format!("{prefix}\u{ffff}");
    let range = web_sys::IdbKeyRange::bound(&JsValue::from_str(prefix), &JsValue::from_str(&upper))
        .map_err(|err| js_err("key range", err))?;

    let request = store
        .open_cursor_with_range(&range)
        .map_err(|err| js_err("opening cursor", err))?;

    let found = Rc::new(RefCell::new(Vec::new()));
    let collected = found.clone();

    let (sender, receiver) = futures_channel();
    let done = Rc::new(RefCell::new(Some(sender)));

    let on_success = {
        let done = done.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let cursor = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::IdbRequest>().ok())
                .and_then(|request| request.result().ok())
                .and_then(|value| value.dyn_into::<web_sys::IdbCursorWithValue>().ok());

            match cursor {
                Some(cursor) => {
                    if let (Ok(key), Ok(value)) = (cursor.key(), cursor.value())
                        && let Some(key) = key.as_string()
                    {
                        let bytes = js_sys::Uint8Array::new(&value).to_vec();
                        collected.borrow_mut().push((key, bytes));
                    }
                    let _ = cursor.continue_();
                }
                None => {
                    if let Some(sender) = done.borrow_mut().take() {
                        sender(Ok(()));
                    }
                }
            }
        })
    };

    let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        if let Some(sender) = done.borrow_mut().take() {
            sender(Err(StoreError::Unavailable("cursor failed".into())));
        }
    });

    request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
    request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_success.forget();
    on_error.forget();

    receiver.await?;
    let entries = std::mem::take(&mut *found.borrow_mut());
    Ok(entries)
}

async fn await_request(request: web_sys::IdbRequest) -> Result<JsValue, StoreError> {
    let (sender, receiver) = futures_channel();
    let done = Rc::new(RefCell::new(Some(sender)));

    let on_success = {
        let done = done.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let value = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::IdbRequest>().ok())
                .and_then(|request| request.result().ok())
                .unwrap_or(JsValue::UNDEFINED);
            if let Some(sender) = done.borrow_mut().take() {
                sender(Ok(value));
            }
        })
    };

    let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        if let Some(sender) = done.borrow_mut().take() {
            sender(Err(StoreError::Unavailable("request failed".into())));
        }
    });

    request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
    request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_success.forget();
    on_error.forget();

    receiver.await
}

fn futures_channel<T: 'static>() -> (
    impl FnOnce(Result<T, StoreError>),
    impl std::future::Future<Output = Result<T, StoreError>>,
) {
    let slot: Rc<RefCell<Option<Result<T, StoreError>>>> = Rc::new(RefCell::new(None));
    let waker: Rc<RefCell<Option<std::task::Waker>>> = Rc::new(RefCell::new(None));

    let send = {
        let slot = slot.clone();
        let waker = waker.clone();
        move |value: Result<T, StoreError>| {
            *slot.borrow_mut() = Some(value);
            if let Some(waker) = waker.borrow_mut().take() {
                waker.wake();
            }
        }
    };

    let future = std::future::poll_fn(move |cx| match slot.borrow_mut().take() {
        Some(value) => std::task::Poll::Ready(value),
        None => {
            *waker.borrow_mut() = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    });

    (send, future)
}
