//! Real-model acceptance and timing check. Run with a GGUF path and optional
//! `cpu` argument. It compares fresh contexts against prefix reuse in one process.
use mikomai_llm_llamacpp::{infer, infer_constrained, inference_stats, load, set_params};
use std::path::Path;

fn run(label: &str, prompt: &str, cached: bool) -> String {
    if cached {
        std::env::remove_var("MIKOMAI_DISABLE_KV_CACHE");
    } else {
        std::env::set_var("MIKOMAI_DISABLE_KV_CACHE", "1");
    }
    let output = infer(prompt).expect("inference must succeed");
    let s = inference_stats();
    println!("{label}: context_reused={} prompt={} reused={} evaluated={} first_token_ms={:.2} total_ms={:.2}",
        s.context_reused, s.prompt_tokens, s.reused_tokens, s.evaluated_tokens, s.first_token_ms, s.total_ms);
    assert!(!output.trim().is_empty());
    if !cached {
        assert!(!s.context_reused);
        assert_eq!(s.reused_tokens, 0);
    }
    output
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let path = args.get(1).expect("performance <GGUF path> [cpu]");
    if args.get(2).is_some_and(|s| s == "cpu") {
        std::env::set_var("MIKOMAI_N_GPU_LAYERS", "0");
    } else {
        std::env::remove_var("MIKOMAI_N_GPU_LAYERS");
    }
    mikomai_llm_llamacpp::logging::configure(true);
    load(Path::new(path)).unwrap();
    set_params(0.0, 1.1, 2048, 32).unwrap();
    let prompt = "VLANとは何ですか。日本語で簡潔に説明してください。";
    let fresh = run("fresh_1", prompt, false);
    let reference = run("fresh_2", prompt, false);
    assert_eq!(fresh, reference, "fresh inference must be deterministic");
    assert_eq!(run("cache_warmup", prompt, true), reference);
    for label in ["cached_1", "cached_2", "cached_3"] {
        let output = run(label, prompt, true);
        assert_eq!(
            reference, output,
            "cached inference must match fresh output"
        );
        let s = inference_stats();
        assert!(s.context_reused);
        assert_eq!(s.evaluated_tokens, 1);
    }
    if args.get(2).is_some_and(|s| s == "cpu") {
        println!("PASS: CPU override and cached/fresh answers");
        return;
    }
    // Changing a suffix must invalidate the old suffix and all generated tokens.
    let changed = "VLANとは何ですか。英語で簡潔に説明してください。";
    let cached_changed = run("changed_cached", changed, true);
    assert!(inference_stats().reused_tokens > 0);
    let fresh_changed = run("changed_fresh", changed, false);
    assert_eq!(cached_changed, fresh_changed);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| infer(prompt).unwrap());
        let second = scope.spawn(|| infer(changed).unwrap());
        assert_eq!(first.join().unwrap(), reference);
        assert_eq!(second.join().unwrap(), fresh_changed);
    });
    // Separate grammar state must never bleed into normal text inference.
    std::env::remove_var("MIKOMAI_DISABLE_KV_CACHE");
    let grammar = "root ::= \"{\\\"ok\\\":true}\"";
    for _ in 0..2 {
        assert_eq!(
            infer_constrained("Return the required JSON.", grammar).unwrap(),
            "{\"ok\":true}"
        );
    }
    assert!(inference_stats().context_reused);
    assert_eq!(run("answer_after_grammar", prompt, true), reference);
    assert!(infer_constrained("Return JSON.", "invalid grammar").is_err());
    assert_eq!(
        infer_constrained("Return the required JSON.", grammar).unwrap(),
        "{\"ok\":true}"
    );
    assert!(
        !inference_stats().context_reused,
        "errors must invalidate the affected slot"
    );
    let caller = std::thread::current().id();
    let mut streamed = String::new();
    let output = mikomai_llm_llamacpp::infer_streaming(prompt, |text, _| {
        assert_eq!(std::thread::current().id(), caller);
        streamed.push_str(text);
    })
    .unwrap();
    assert_eq!(streamed.trim(), output);
    assert_eq!(output, reference);
    mikomai_llm_llamacpp::CANCEL_INFERENCE.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(infer(prompt).unwrap(), "生成を停止しました。");
    mikomai_llm_llamacpp::CANCEL_INFERENCE.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(run("after_cancel", prompt, true), reference);
    assert!(!inference_stats().context_reused);
    set_params(0.0, 1.1, 1024, 32).unwrap();
    run("context_changed", prompt, true);
    assert!(!inference_stats().context_reused);
    load(Path::new(path)).unwrap();
    run("model_reloaded", prompt, true);
    assert!(!inference_stats().context_reused);
    println!("PASS: cached/fresh answers, changed suffix, grammar isolation, context change, model reload");
}
