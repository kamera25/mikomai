use mikomai_adapters::headless::{EchoToolExecutor, JsonTaskRepository, StdoutReporter};
use mikomai_adapters::knowledge::{KnowledgePlanner, KnowledgeStore};
use mikomai_core::application::ChatService;
use mikomai_core::TaskManager;
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;

#[repr(C)]
pub struct MikomaiResult {
    pub status: i32,
    pub message: *mut c_char,
}

/// Runs the same local knowledge chat flow as `mikomai-cli chat`.
/// `message` must point to a valid, NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn mikomai_chat(message: *const c_char) -> MikomaiResult {
    if message.is_null() {
        return error_result("chat message must not be null".into());
    }

    let caught = std::panic::catch_unwind(|| {
        let goal = CStr::from_ptr(message)
            .to_str()
            .map_err(|error| error.to_string())?;
        chat(goal)
    });

    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("mikomai chat failed unexpectedly".into()),
    }
}

/// Runs local knowledge chat using explicitly selected document and index directories.
/// The two directory arguments must be valid, NUL-terminated UTF-8 strings.
#[no_mangle]
pub unsafe extern "C" fn mikomai_chat_with_paths(
    message: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
) -> MikomaiResult {
    if message.is_null() || documents_dir.is_null() || knowledge_dir.is_null() {
        return error_result("chat message and directories must not be null".into());
    }

    let caught = std::panic::catch_unwind(|| {
        let goal = CStr::from_ptr(message)
            .to_str()
            .map_err(|error| error.to_string())?;
        let documents = CStr::from_ptr(documents_dir)
            .to_str()
            .map_err(|error| error.to_string())?;
        let knowledge = CStr::from_ptr(knowledge_dir)
            .to_str()
            .map_err(|error| error.to_string())?;
        chat_with_paths(goal, PathBuf::from(documents), PathBuf::from(knowledge))
    });

    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("mikomai chat failed unexpectedly".into()),
    }
}

/// Releases the message returned in `MikomaiResult`.
#[no_mangle]
pub unsafe extern "C" fn mikomai_result_free(result: MikomaiResult) {
    if !result.message.is_null() {
        drop(CString::from_raw(result.message));
    }
}

fn chat(goal: &str) -> Result<String, String> {
    let knowledge_root = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    let documents = std::env::var_os("MIKOMAI_DOCS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("nw-docs"));
    chat_with_paths(goal, documents, knowledge_root)
}

fn chat_with_paths(
    goal: &str,
    documents: PathBuf,
    knowledge_root: PathBuf,
) -> Result<String, String> {
    if goal.trim().is_empty() {
        return Err("chat message is required".into());
    }

    if !documents.is_dir() {
        return Err(format!(
            "documents directory is missing or is not a directory: {}",
            documents.display()
        ));
    }
    let store = KnowledgeStore::at(knowledge_root);
    store.ingest(documents)?;

    let manager = TaskManager::new(JsonTaskRepository::default());
    let planner = KnowledgePlanner::new(&store);
    let executor = EchoToolExecutor;
    let reporter = StdoutReporter::default();
    let task = manager.start(goal).map_err(|error| error.to_string())?;
    let service = ChatService::new(&planner, &executor, &reporter);
    futures_lite::future::block_on(manager.run_chat(&service, task))
}

fn error_result(message: String) -> MikomaiResult {
    result(1, message)
}

fn result(status: i32, message: String) -> MikomaiResult {
    let message = CString::new(message.replace('\0', "�"))
        .unwrap_or_else(|_| CString::new("mikomai returned invalid text").unwrap());
    MikomaiResult {
        status,
        message: message.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::{mikomai_chat, mikomai_chat_with_paths, mikomai_result_free};
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn c_abi_reports_errors_answers_from_local_documents_and_frees_results() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mikomai-ffi-test-{unique}"));
        let docs = root.join("docs");
        let index = root.join("index");
        fs::create_dir_all(&docs).expect("create temporary docs directory");
        fs::write(
            docs.join("acceptance.md"),
            "# F220 VLAN acceptance\n\nUnique answer marker: NATIVE-FFI-ANSWER-7319.",
        )
        .expect("write temporary knowledge document");

        let old_docs = std::env::var_os("MIKOMAI_DOCS_DIR");
        let old_index = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR");
        std::env::set_var("MIKOMAI_DOCS_DIR", &docs);
        std::env::set_var("MIKOMAI_KNOWLEDGE_DIR", &index);

        unsafe {
            let null_result = mikomai_chat(std::ptr::null());
            assert_eq!(null_result.status, 1);
            assert!(CStr::from_ptr(null_result.message)
                .to_string_lossy()
                .contains("must not be null"));
            mikomai_result_free(null_result);

            let empty = CString::new("").unwrap();
            let empty_result = mikomai_chat(empty.as_ptr());
            assert_eq!(empty_result.status, 1);
            assert!(CStr::from_ptr(empty_result.message)
                .to_string_lossy()
                .contains("chat message is required"));
            mikomai_result_free(empty_result);

            let question = CString::new("F220 VLAN").unwrap();
            let answer = mikomai_chat(question.as_ptr());
            assert_eq!(answer.status, 0);
            assert!(CStr::from_ptr(answer.message)
                .to_string_lossy()
                .contains("NATIVE-FFI-ANSWER-7319"));
            mikomai_result_free(answer);

            let empty_docs = root.join("empty-docs");
            let empty_index = root.join("empty-index");
            fs::create_dir_all(&empty_docs).expect("create unrelated docs directory");
            std::env::set_var("MIKOMAI_DOCS_DIR", &empty_docs);
            std::env::set_var("MIKOMAI_KNOWLEDGE_DIR", &empty_index);
            let configured = mikomai_chat_with_paths(
                question.as_ptr(),
                CString::new(docs.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
                CString::new(index.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
            );
            assert_eq!(configured.status, 0);
            assert!(CStr::from_ptr(configured.message)
                .to_string_lossy()
                .contains("NATIVE-FFI-ANSWER-7319"));
            mikomai_result_free(configured);
            std::env::set_var("MIKOMAI_DOCS_DIR", &docs);
            std::env::set_var("MIKOMAI_KNOWLEDGE_DIR", &index);

            let missing_docs =
                CString::new(root.join("missing-docs").to_string_lossy().as_bytes()).unwrap();
            let configured_missing = mikomai_chat_with_paths(
                question.as_ptr(),
                missing_docs.as_ptr(),
                CString::new(index.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
            );
            assert_eq!(configured_missing.status, 1);
            assert!(CStr::from_ptr(configured_missing.message)
                .to_string_lossy()
                .contains("documents directory is missing"));
            mikomai_result_free(configured_missing);
        }

        restore_env("MIKOMAI_DOCS_DIR", old_docs);
        restore_env("MIKOMAI_KNOWLEDGE_DIR", old_index);
        fs::remove_dir_all(root).expect("remove temporary FFI test data");
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}
