use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    js_sys::{ArrayBuffer, Uint8Array},
    Request, RequestInit, RequestMode, Response,
};

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
};

use super::{Js5Index, Js5ResourceProvider};
use bytes::{Bytes, BytesMut};

enum Js5RequestDataState {
    NotLoaded,
    Loading(BytesMut),
    Loaded(Bytes),
}

impl Default for Js5RequestDataState {
    fn default() -> Self {
        Self::NotLoaded
    }
}

pub struct Js5Request {
    pub archive_id: u8,
    pub group_id: u32,
    urgent: bool,
    cached: bool,
    completed: AtomicBool,
    orphaned: AtomicBool,
    data: Mutex<Js5RequestDataState>,
}

impl Js5Request {
    pub fn new(archive_id: u8, group_id: u32, urgent: bool, cached: bool) -> Self {
        Self {
            archive_id,
            group_id,
            urgent,
            cached,
            completed: AtomicBool::new(false),
            orphaned: AtomicBool::new(false),
            data: Mutex::new(Js5RequestDataState::NotLoaded),
        }
    }

    #[inline]
    pub fn is_urgent(&self) -> bool {
        self.urgent
    }

    #[inline]
    pub fn is_cached(&self) -> bool {
        self.cached
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        self.completed.load(Ordering::Acquire)
    }

    #[inline]
    pub fn mark_complete(&self) {
        self.completed.store(true, Ordering::Release);
    }

    #[inline]
    pub fn is_orphaned(&self) -> bool {
        self.orphaned.load(Ordering::Acquire)
    }

    #[inline]
    pub fn mark_orphaned(&self) {
        self.orphaned.store(true, Ordering::Release);
    }

    pub fn init_data(&self, data: BytesMut) {
        let mut req_data = self.data.lock().unwrap();
        *req_data = Js5RequestDataState::Loading(data);
    }

    pub fn complete_data(&self, data: Bytes) {
        let mut req_data = self.data.lock().unwrap();
        *req_data = Js5RequestDataState::Loaded(data);
    }

    pub fn get_data(&self) -> Option<Bytes> {
        let mut req_data = self.data.lock().unwrap();
        match &*req_data {
            Js5RequestDataState::Loading(_) if self.is_completed() => {
                if let Js5RequestDataState::Loading(data) = std::mem::take(&mut *req_data) {
                    let data = data.freeze();
                    *req_data = Js5RequestDataState::Loaded(data.clone());
                    Some(data)
                } else {
                    panic!("Invalid state");
                }
            }
            Js5RequestDataState::Loaded(data) => Some(data.clone()),
            _ => None,
        }
    }
}

pub struct Openrs2Js5ResourceProviderState {
    index: Option<Arc<Js5Index>>,
    index_request: Option<Arc<Js5Request>>,
    requests: HashMap<u32, Arc<Js5Request>>,
}

impl Openrs2Js5ResourceProviderState {
    pub fn new(index_request: Option<Arc<Js5Request>>) -> Self {
        Self {
            index: None,
            index_request,
            requests: HashMap::new(),
        }
    }
}

pub struct Openrs2Js5ResourceProvider {
    archive_id: u8,
    net_client: Arc<Openrs2Js5NetClient>,
    state: Mutex<Openrs2Js5ResourceProviderState>,
}

impl Openrs2Js5ResourceProvider {
    pub fn new(archive_id: u8, net_client: Arc<Openrs2Js5NetClient>) -> Self {
        let index_request = Self::request_index(&net_client, archive_id);
        Self {
            archive_id,
            net_client,
            state: Mutex::new(Openrs2Js5ResourceProviderState::new(index_request)),
        }
    }

    fn request_index(net_client: &Openrs2Js5NetClient, archive_id: u8) -> Option<Arc<Js5Request>> {
        net_client.queue_request(Js5Index::ARCHIVE_ID, archive_id as u32, true)
    }
}

impl Js5ResourceProvider for Openrs2Js5ResourceProvider {
    fn fetch_index(&self) -> Option<Arc<Js5Index>> {
        let mut state = self.state.lock().unwrap();
        if let Some(index) = &state.index {
            return Some(index.clone());
        }
        let request = if let Some(request) = &state.index_request {
            request.clone()
        } else {
            let request = Self::request_index(&self.net_client, self.archive_id)?;
            state.index_request = Some(request.clone());
            request
        };

        if !request.is_completed() {
            return None;
        }

        if let Some(data) = request.get_data() {
            let mut index = Js5Index::decode(&data, None);
            index.clear_data_sizes();

            // if !request.is_cached() {
            //     self.disk_cache.queue_write_index(
            //         data,
            //         index.version,
            //         index.crc,
            //         self.store.clone(),
            //     );
            // }

            let index = Arc::new(index);

            state.index = Some(index.clone());
            // state.group_status = vec![Js5GroupStatus::NotLoaded; index.group_capacity as usize];
            // state.verified_groups = 0;

            state.index_request = None;

            Some(index)
        } else {
            state.index_request = None;

            None
        }
    }

    fn fetch_group(&self, group_id: u32) -> Option<Bytes> {
        let mut state = self.state.lock().unwrap();

        let request = match state.requests.entry(group_id) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let request = self
                    .net_client
                    .queue_request(self.archive_id, group_id, true)?;
                entry.insert(request.clone());
                request
            }
        };

        if !request.is_completed() {
            return None;
        }

        let request = request.clone();

        state.requests.remove(&group_id);

        request.get_data()
    }
}

#[wasm_bindgen(module = "/src/test.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn fetch_group(archive_id: u8, group_id: u32) -> Result<JsValue, JsValue>;
}

pub struct Openrs2Js5NetClient {
    cache_id: u32,
    queued_request_count: Arc<AtomicU32>,
}

impl Openrs2Js5NetClient {
    pub fn new(cache_id: u32) -> Self {
        Self {
            cache_id,
            queued_request_count: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn queue_request(
        &self,
        archive_id: u8,
        group_id: u32,
        urgent: bool,
    ) -> Option<Arc<Js5Request>> {
        if self.queued_request_count.load(Ordering::Acquire) >= 20 {
            return None;
        }

        self.queued_request_count.fetch_add(1, Ordering::Release);

        let request = Arc::new(Js5Request::new(archive_id, group_id, urgent, false));

        wasm_bindgen_futures::spawn_local({
            let cache_id = self.cache_id;
            let request = request.clone();
            let queued_request_count = self.queued_request_count.clone();
            async move {
                match Self::fetch(cache_id, archive_id, group_id).await {
                    Ok(data) => {
                        request.complete_data(data);
                        request.mark_complete();
                    }
                    Err(e) => {
                        log::error!("Failed to fetch group: {:?}", e);
                        request.mark_complete();
                    }
                }
                queued_request_count.fetch_sub(1, Ordering::Release);
            }
        });

        Some(request)
    }

    pub async fn fetch(cache_id: u32, archive_id: u8, group_id: u32) -> Result<Bytes, JsValue> {
        let array_buffer = fetch_group(archive_id, group_id).await?;
        assert!(array_buffer.is_instance_of::<ArrayBuffer>());
        let typed_array = Uint8Array::new(&array_buffer);
        let mut data = vec![0; typed_array.length() as usize];
        typed_array.copy_to(&mut data);

        Ok(Bytes::from(data))
    }
}

async fn run(repo: String) -> Result<JsValue, JsValue> {
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let url = format!("https://api.github.com/repos/{}/branches/master", repo);

    let request = Request::new_with_str_and_init(&url, &opts)?;

    request
        .headers()
        .set("Accept", "application/vnd.github.v3+json")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    // `resp_value` is a `Response` object.
    assert!(resp_value.is_instance_of::<Response>());
    let resp: Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let json = JsFuture::from(resp.json()?).await?;

    // Send the JSON response back to JS.
    Ok(json)
}
