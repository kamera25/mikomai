//! A single native inference thread owns the model and its borrowed contexts.
//! This keeps llama contexts on their creating thread, without extending
//! lifetimes or declaring native pointers Send. Callbacks run on the caller.
use super::*;
use std::sync::mpsc::{self, Receiver, Sender};

enum Request {
    Load(PathBuf, Sender<Result<String, String>>),
    Shutdown(Sender<()>),
    Infer {
        prompt: String,
        vision: Option<mikomai_core::vision::VisionRequest>,
        grammar: Option<String>,
        reply: Sender<Event>,
    },
}

// Register after native backend/model initialization, so this runs before
// llama.cpp's C++ static Metal-device destructors (atexit order is reversed).
// Retained contexts must be freed before those devices assert no buffers remain.
extern "C" {
    fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
}

extern "C" fn shutdown_at_exit() {
    if let Ok(worker) = worker() {
        let (tx, rx) = mpsc::channel();
        if worker.send(Request::Shutdown(tx)).is_ok() {
            let _ = rx.recv();
        }
    }
}

fn register_shutdown() -> Result<(), String> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            // SAFETY: atexit takes a static C function pointer; this callback never
            // unwinds and only asks the owning thread to drop its native resources.
            if unsafe { atexit(shutdown_at_exit) } == 0 {
                Ok(())
            } else {
                Err("could not register inference worker shutdown".into())
            }
        })
        .clone()
}
enum Event {
    Token(String, bool),
    Done(Result<String, String>),
}

fn worker() -> Result<&'static Sender<Request>, String> {
    static WORKER: OnceLock<Result<Sender<Request>, String>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("mikomai-llama".into())
                .spawn(move || run(rx))
                .map(|_| tx)
                .map_err(|e| format!("inference worker initialization failed: {e}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) fn load(path: &Path) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    worker()?
        .send(Request::Load(path.to_path_buf(), tx))
        .map_err(|_| "inference worker stopped")?;
    rx.recv().map_err(|_| "inference worker stopped")?
}

pub(super) fn infer<F: FnMut(&str, bool)>(
    prompt: &str,
    vision: Option<&mikomai_core::vision::VisionRequest>,
    grammar: Option<&str>,
    mut on_token: F,
) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    worker()?
        .send(Request::Infer {
            prompt: prompt.into(),
            vision: vision.map(|v| mikomai_core::vision::VisionRequest {
                prompt: v.prompt.clone(),
                images: v.images.clone(),
            }),
            grammar: grammar.map(str::to_owned),
            reply: tx,
        })
        .map_err(|_| "inference worker stopped")?;
    while let Ok(event) = rx.recv() {
        match event {
            Event::Token(text, done) => on_token(&text, done),
            Event::Done(result) => return result,
        }
    }
    Err("inference worker stopped".into())
}

fn load_native(backend: Arc<LlamaBackend>, path: PathBuf) -> Result<LoadedModel, String> {
    let explicit = std::env::var("MIKOMAI_N_GPU_LAYERS").ok();
    let layers = gpu_layers(explicit.as_deref(), backend.supports_gpu_offload())?;
    let mut params = LlamaModelParams::default().with_n_gpu_layers(layers);
    if layers == 0 {
        params = params.with_devices(&[]).map_err(|e| e.to_string())?;
    }
    let params = std::pin::pin!(params);
    let model = LlamaModel::load_from_file(&backend, &path, &params)
        .map_err(|e| format!("model load failed: {e}"))?;
    Ok(LoadedModel {
        backend,
        model: Arc::new(model),
        path,
        gpu_layers: layers,
    })
}

fn run(rx: Receiver<Request>) {
    let mut backend = None;
    let mut loaded = None;
    let mut pending = None;
    loop {
        let request = match pending.take().or_else(|| rx.recv().ok()) {
            Some(request) => request,
            None => return,
        };
        match request {
            Request::Load(path, reply) => {
                let result = (|| {
                    if backend.is_none() {
                        backend = Some(Arc::new(LlamaBackend::init().map_err(|e| {
                            format!("llama.cpp backend initialization failed: {e}")
                        })?));
                    }
                    let next = load_native(backend.as_ref().unwrap().clone(), path)?;
                    register_shutdown()?;
                    *model_path_slot()
                        .lock()
                        .map_err(|_| "model state is unavailable")? = Some(next.path.clone());
                    loaded = Some(next);
                    CANCEL_INFERENCE.store(false, Ordering::Relaxed);
                    Ok("モデルを読み込みました".into())
                })();
                let _ = reply.send(result);
            }
            Request::Shutdown(reply) => {
                // All borrowed contexts have already dropped on leaving the
                // inner loop. Model must drop before backend, then acknowledge.
                drop(loaded.take());
                drop(backend.take());
                if let Ok(mut path) = model_path_slot().lock() {
                    *path = None;
                }
                let _ = reply.send(());
                return;
            }
            request @ Request::Infer { .. } => {
                let Some(model) = loaded.as_ref() else {
                    if let Request::Infer { reply, .. } = request {
                        let _ = reply.send(Event::Done(Err(
                            "モデルが未ロードです。設定から GGUF モデルを読み込んでください。"
                                .into(),
                        )));
                    }
                    continue;
                };
                // Two bounded slots keep answer and constrained agent prefixes
                // separate. Vision is never token-only cached.
                let mut answer = None;
                let mut constrained = None;
                let mut next = Some(request);
                loop {
                    let request = match next.take().or_else(|| rx.recv().ok()) {
                        Some(request) => request,
                        None => return,
                    };
                    match request {
                        control @ (Request::Load(..) | Request::Shutdown(..)) => {
                            pending = Some(control);
                            break;
                        }
                        Request::Infer {
                            prompt,
                            vision,
                            grammar,
                            reply,
                        } => {
                            let mut vision_context = None;
                            let cache = if vision.is_some() {
                                &mut vision_context
                            } else if grammar.is_some() {
                                &mut constrained
                            } else {
                                &mut answer
                            };
                            let result = infer_loaded(
                                model,
                                cache,
                                &prompt,
                                vision.as_ref(),
                                grammar.as_deref(),
                                |text, done| {
                                    let _ = reply.send(Event::Token(text.to_owned(), done));
                                },
                            );
                            // On errors, no partially decoded prefix is reusable.
                            if result.is_err() || CANCEL_INFERENCE.load(Ordering::Relaxed) {
                                *cache = None;
                            }
                            let _ = reply.send(Event::Done(result));
                        }
                    }
                }
                // Borrowed contexts drop here, before model replacement.
            }
        }
    }
}
