//! Real mtmd smoke check after a llama.cpp dependency update.
use mikomai_core::port::VisionPort;
use mikomai_core::vision::{ImageInput, VisionRequest};
use mikomai_llm_llamacpp::{configure_vision, load, set_params, LocalVision};
use std::path::Path;
use std::task::{Context, Poll, Waker};

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 4, "vision_smoke <model.gguf> <mmproj.gguf> <red.png>");
    mikomai_llm_llamacpp::logging::configure(true);
    load(Path::new(&args[1])).unwrap();
    configure_vision(true, Some(Path::new(&args[2]))).unwrap();
    set_params(0.0, 1.1, 4096, 64).unwrap();
    let request = VisionRequest {
        prompt: "画像全体の色を、日本語で一言で答えてください。".into(),
        images: vec![ImageInput { name: "sample.png".into(), mime_type: "image/png".into(), data: std::fs::read(&args[3]).unwrap() }],
    };
    let vision = LocalVision;
    let mut future = vision.analyze(&request);
    let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
        panic!("local vision inference unexpectedly suspended");
    };
    let answer = result.unwrap();
    println!("{answer}");
    assert!(answer.contains('赤'), "solid red image must be identified as red");
}
