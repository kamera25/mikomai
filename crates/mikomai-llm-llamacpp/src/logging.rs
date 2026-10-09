use llama_cpp_sys_2::{ggml_log_level, GGML_LOG_LEVEL_ERROR, GGML_LOG_LEVEL_WARN};
use std::ffi::{c_char, c_void, CStr};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG: AtomicBool = AtomicBool::new(false);

pub(crate) fn inference_stats(stats: &crate::InferenceStats) {
    if DEBUG.load(Ordering::Relaxed) {
        let _ = writeln!(std::io::stderr().lock(),
            "mikomai llama: context_reused={} prompt_tokens={} reused_tokens={} evaluated_tokens={} first_token_ms={:.2} total_ms={:.2}",
            stats.context_reused, stats.prompt_tokens, stats.reused_tokens,
            stats.evaluated_tokens, stats.first_token_ms, stats.total_ms);
    }
}

pub fn configure(debug: bool) {
    DEBUG.store(debug, Ordering::Relaxed);
    // Install before backend initialization/model loading. This callback is
    // process-wide, but only the CLI installs it; desktop logging is unchanged.
    unsafe {
        llama_cpp_sys_2::llama_log_set(Some(llama_log), std::ptr::null_mut());
    }
}

unsafe extern "C" fn llama_log(level: ggml_log_level, text: *const c_char, _: *mut c_void) {
    if text.is_null() {
        return;
    }
    let text = CStr::from_ptr(text).to_bytes();
    write_log(
        &mut std::io::stderr().lock(),
        DEBUG.load(Ordering::Relaxed),
        level,
        text,
    );
}

fn write_log(writer: &mut impl Write, debug: bool, level: ggml_log_level, text: &[u8]) {
    let model_setup_notice = level == GGML_LOG_LEVEL_WARN
        && (text.starts_with(b"load: control-looking token:")
            || text.starts_with(b"load: special_eog_ids contains '<|tool_response>', removing '</s>' token from EOG list")
            || text.starts_with(b"llama_kv_cache_iswa: using full-size SWA cache (ref:"));
    if debug || (matches!(level, GGML_LOG_LEVEL_WARN | GGML_LOG_LEVEL_ERROR) && !model_setup_notice)
    {
        // A closed stderr must not panic across the native callback boundary.
        let _ = writer.write_all(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llama_cpp_sys_2::{GGML_LOG_LEVEL_CONT, GGML_LOG_LEVEL_DEBUG, GGML_LOG_LEVEL_INFO};

    #[test]
    fn tensor_loading_logs_require_debug() {
        let text = b"create_tensor: loading tensor blk.39.attn_k.input_scale\n";
        let mut output = Vec::new();
        write_log(&mut output, false, GGML_LOG_LEVEL_DEBUG, text);
        assert!(output.is_empty());
        write_log(&mut output, true, GGML_LOG_LEVEL_DEBUG, text);
        assert_eq!(output, text);
    }

    #[test]
    fn model_info_and_loading_progress_require_debug() {
        for (level, text) in [
            (
                GGML_LOG_LEVEL_INFO,
                b"load_tensors: loading model tensors\n".as_slice(),
            ),
            (
                GGML_LOG_LEVEL_INFO,
                b"ggml_metal_device_init: tensor API disabled\n".as_slice(),
            ),
            (GGML_LOG_LEVEL_CONT, b".".as_slice()),
        ] {
            let mut output = Vec::new();
            write_log(&mut output, false, level, text);
            assert!(output.is_empty());
            write_log(&mut output, true, level, text);
            assert_eq!(output, text);
        }
    }

    #[test]
    fn warnings_and_errors_are_preserved() {
        for level in [GGML_LOG_LEVEL_WARN, GGML_LOG_LEVEL_ERROR] {
            let mut output = Vec::new();
            write_log(&mut output, false, level, b"model diagnostic\n");
            assert_eq!(output, b"model diagnostic\n");
        }
    }

    #[test]
    fn model_setup_notices_require_debug_but_errors_remain_visible() {
        for text in [
            "load: control-looking token:     50 '<|tool_response>' was not control-type; this is probably a bug in the model. its type will be overridden\n",
            "load: control-looking token:    212 '</s>' was not control-type; this is probably a bug in the model. its type will be overridden\n",
            "load: control-looking token:      1 '<eos>' was not control-type; this is probably a bug in the model. its type will be overridden\n",
            "load: special_eog_ids contains '<|tool_response>', removing '</s>' token from EOG list\n",
            "llama_kv_cache_iswa: using full-size SWA cache (ref: https://github.com/ggml-org/llama.cpp/pull/13194#issuecomment-2868343055)\n",
        ] {
            let mut output = Vec::new();
            write_log(&mut output, false, GGML_LOG_LEVEL_WARN, text.as_bytes());
            assert!(output.is_empty());
            write_log(&mut output, true, GGML_LOG_LEVEL_WARN, text.as_bytes());
            assert_eq!(output, text.as_bytes());
            output.clear();
            write_log(&mut output, false, GGML_LOG_LEVEL_ERROR, text.as_bytes());
            assert_eq!(output, text.as_bytes());
        }
    }
}
