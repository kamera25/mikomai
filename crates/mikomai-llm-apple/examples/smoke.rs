#[cfg(target_os = "macos")]
fn main() -> Result<(), String> {
    use mikomai_llm::{InferencePort, ModelAvailability};
    use mikomai_llm_apple::AppleInference;
    let backend = AppleInference::default();
    let availability = backend.availability();
    println!("Availability: {availability:?}");
    println!("Capabilities: {:?}", backend.capabilities());
    if availability != ModelAvailability::Available {
        return Err("Apple model is unavailable".into());
    }
    let response =
        futures_lite::future::block_on(backend.complete("日本語で短く挨拶してください。"))?;
    println!("{response}");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {}
