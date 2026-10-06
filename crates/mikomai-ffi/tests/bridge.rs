//! Exercise the exported ABI after moving its implementation to the app crate.
use mikomai_ffi::*;
use std::ffi::{CStr, CString};

fn consume(result: MikomaiResult) -> (i32, String) {
    let text = unsafe { CStr::from_ptr(result.message) }
        .to_string_lossy()
        .into_owned();
    let status = result.status;
    unsafe { mikomai_result_free(result) };
    (status, text)
}

#[test]
fn bridge_and_app_share_approval_state_and_result_ownership() {
    let root = std::env::temp_dir().join(format!("mikomai-bridge-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("MIKOMAI_OPERATION_PLANS_PATH", root.join("plans.json"));
    std::env::set_var("MIKOMAI_DATA_DIR", &root);
    let target = CString::new("bridge-fixture").unwrap();
    let snapshot = CString::new(r#"{"id":"fixture","host":"192.0.2.1"}"#).unwrap();
    let commands = CString::new(r#"["hostname bridge-fixture"]"#).unwrap();
    let reason = CString::new("bridge compatibility check").unwrap();
    let (status, text) = consume(unsafe {
        mikomai_operation_plan_create(
            target.as_ptr(),
            snapshot.as_ptr(),
            commands.as_ptr(),
            reason.as_ptr(),
        )
    });
    assert_eq!(status, 0, "{text}");
    let plan: serde_json::Value = serde_json::from_str(&text).unwrap();
    let id = CString::new(plan["id"].as_str().unwrap()).unwrap();
    let hash = CString::new(plan["planHash"].as_str().unwrap()).unwrap();
    let (status, fetched) =
        consume(unsafe { mikomai_app::mikomai_operation_plan_get(id.as_ptr()) });
    assert_eq!(status, 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fetched).unwrap(),
        plan
    );
    assert_eq!(
        consume(unsafe { mikomai_operation_plan_begin(id.as_ptr(), hash.as_ptr()) }).0,
        1
    );
    assert_eq!(
        consume(unsafe { mikomai_app::mikomai_operation_plan_approve(id.as_ptr(), hash.as_ptr()) })
            .0,
        0
    );
    assert_eq!(
        consume(unsafe { mikomai_operation_plan_begin(id.as_ptr(), hash.as_ptr()) }).0,
        0
    );
    assert_eq!(
        consume(unsafe { mikomai_operation_plan_begin(id.as_ptr(), hash.as_ptr()) }).0,
        1
    );
    assert_eq!(consume(unsafe { mikomai_chat(std::ptr::null()) }).0, 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_policy_bridge_preserves_scalar_results_and_rejects_imports() {
    let input = CString::new(r#"{"op":"dry_run","processSucceeded":true,"output":"{\"success\":true,\"results\":[{\"ok\":true}]}"}"#).unwrap();
    let (status, text) = consume(unsafe { mikomai_native_query(input.as_ptr()) });
    assert_eq!(status, 0);
    assert_eq!(text, "true");
    let input = CString::new(r#"{"op":"import_json","connections":[]}"#).unwrap();
    assert_eq!(
        consume(unsafe { mikomai_native_query(input.as_ptr()) }).0,
        1
    );
    assert_eq!(
        consume(unsafe { mikomai_native_query(std::ptr::null()) }).0,
        1
    );
}
