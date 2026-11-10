use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Blob, BlobEvent, HtmlCanvasElement, MediaRecorder, MediaRecorderOptions, MediaStreamTrack,
};

use crate::platform::time::Instant;

const FORMATS: [(&str, &str); 4] = [
    ("video/webm;codecs=vp9", "webm"),
    ("video/webm;codecs=vp8", "webm"),
    ("video/webm", "webm"),
    ("video/mp4", "mp4"),
];

const MAX_FPS: f64 = 30.0;

const VIDEO_BITS_PER_SECOND: u32 = 8_000_000;

const SLICE_MS: i32 = 1000;

struct Active {
    recorder: MediaRecorder,
    started: Instant,
    stopping: bool,
    _on_data: Closure<dyn FnMut(BlobEvent)>,
    _on_error: Closure<dyn FnMut(JsValue)>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy)]
pub(crate) enum Status {
    Idle,
    Recording { secs: u64 },
    Saving,
}

pub(crate) fn status() -> Status {
    ACTIVE.with_borrow(|a| match a {
        None => Status::Idle,
        Some(a) if a.stopping => Status::Saving,
        Some(a) => Status::Recording {
            secs: a.started.elapsed().as_secs(),
        },
    })
}

pub(crate) fn last_error() -> Option<String> {
    LAST_ERROR.with_borrow(Clone::clone)
}

pub(crate) fn start() {
    if ACTIVE.with_borrow(Option::is_some) {
        return;
    }
    LAST_ERROR.set(None);
    match begin() {
        Ok(active) => {
            crate::log_info!("record", "recording started");
            ACTIVE.set(Some(active));
        }
        Err(e) => fail(format!("could not start recording: {e}")),
    }
}

pub(crate) fn stop() {
    ACTIVE.with_borrow_mut(|a| {
        let Some(a) = a.as_mut().filter(|a| !a.stopping) else {
            return;
        };
        a.stopping = true;
        if let Err(e) = a.recorder.stop() {
            crate::log_warn!("record", "stop failed: {}", super::describe_js(&e));
        }
    });
}

fn fail(message: String) {
    crate::log_warn!("record", "{message}");
    LAST_ERROR.set(Some(message));
}

fn begin() -> Result<Active, String> {
    let js = |e: JsValue| super::describe_js(&e);
    let (mime, ext) = FORMATS
        .into_iter()
        .find(|(mime, _)| MediaRecorder::is_type_supported(mime))
        .ok_or("this browser cannot encode video")?;

    let canvas: HtmlCanvasElement = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("canvas"))
        .ok_or("no canvas")?
        .dyn_into()
        .map_err(|_| "the canvas element is not a canvas")?;
    let stream = canvas
        .capture_stream_with_frame_request_rate(MAX_FPS)
        .map_err(js)?;

    let options = MediaRecorderOptions::new();
    options.set_mime_type(mime);
    options.set_video_bits_per_second(VIDEO_BITS_PER_SECOND);
    let recorder =
        MediaRecorder::new_with_media_stream_and_media_recorder_options(&stream, &options)
            .map_err(js)?;

    let chunks: Rc<RefCell<Vec<Blob>>> = Rc::default();
    let on_data = {
        let chunks = chunks.clone();
        Closure::<dyn FnMut(BlobEvent)>::new(move |event: BlobEvent| {
            if let Some(blob) = event.data().filter(|b| b.size() > 0.0) {
                chunks.borrow_mut().push(blob);
            }
        })
    };
    let on_error = Closure::<dyn FnMut(JsValue)>::new(|event: JsValue| {
        fail(format!(
            "the recorder failed: {}",
            super::describe_js(&event)
        ));
    });

    let name = format!(
        "{}.{ext}",
        super::screenshot::timestamp(super::screenshot::epoch_secs())
    );
    let mime_type = mime.to_string();
    let on_stop = Closure::once_into_js(move || {
        let recorder = ACTIVE.take().map(|a| a.recorder);
        if let Some(recorder) = &recorder {
            release(recorder);
        }
        let parts: js_sys::Array = chunks.borrow().iter().collect();
        let result = (|| {
            let options = web_sys::BlobPropertyBag::new();
            options.set_type(&mime_type);
            let blob = Blob::new_with_blob_sequence_and_options(&parts, &options)
                .map_err(|e| super::describe_js(&e))?;
            if blob.size() == 0.0 {
                return Err("nothing was recorded".to_string());
            }
            super::download::save_blob(&name, &blob)
        })();
        match result {
            Ok(()) => crate::log_info!("record", "saved {name}"),
            Err(e) => fail(format!("could not save the recording: {e}")),
        }
    });

    recorder.set_ondataavailable(Some(on_data.as_ref().unchecked_ref()));
    recorder.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    recorder.set_onstop(Some(on_stop.unchecked_ref()));
    if let Err(e) = recorder.start_with_time_slice(SLICE_MS) {
        release(&recorder);
        return Err(js(e));
    }

    Ok(Active {
        recorder,
        started: Instant::now(),
        stopping: false,
        _on_data: on_data,
        _on_error: on_error,
    })
}

fn release(recorder: &MediaRecorder) {
    for track in recorder.stream().get_tracks().iter() {
        if let Ok(track) = track.dyn_into::<MediaStreamTrack>() {
            track.stop();
        }
    }
    recorder.set_ondataavailable(None);
    recorder.set_onerror(None);
    recorder.set_onstop(None);
}
