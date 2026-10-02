use llama_cpp_sys_2::{ggml_log_level, GGML_LOG_LEVEL_ERROR, GGML_LOG_LEVEL_WARN};
use std::ffi::{c_char, c_void, CStr};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG: AtomicBool = AtomicBool::new(false);

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
    if debug || matches!(level, GGML_LOG_LEVEL_WARN | GGML_LOG_LEVEL_ERROR) {
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
}
