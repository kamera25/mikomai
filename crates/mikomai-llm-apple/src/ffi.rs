//! The only unsafe Rust boundary. Calls are synchronous; input pointers are
//! borrowed for the call, handles are retained until destroy, and every returned
//! string is an owned allocation released with the bridge's string_free.
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::{self, NonNull};

extern "C" {
    fn mikomai_fm_session_create(
        instructions: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn mikomai_fm_session_respond(
        session: *mut c_void,
        prompt: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut c_char;
    fn mikomai_fm_session_destroy(session: *mut c_void);
    fn mikomai_fm_string_free(value: *mut c_char);
}

/// Deliberately !Send and !Sync: created, used, and destroyed on one worker.
pub(crate) struct Session(NonNull<c_void>);

struct NativeString(NonNull<c_char>);
impl NativeString {
    fn take(ptr: *mut c_char) -> Option<Self> {
        NonNull::new(ptr).map(Self)
    }

    fn to_rust(&self) -> Result<String, String> {
        // SAFETY: bridge allocations are valid NUL-terminated strings until Drop.
        unsafe { CStr::from_ptr(self.0.as_ptr()) }
            .to_str()
            .map(str::to_owned)
            .map_err(|e| format!("Invalid UTF-8 from Apple Foundation Models: {e}"))
    }
}
impl Drop for NativeString {
    fn drop(&mut self) {
        // SAFETY: this wrapper uniquely owns this bridge allocation.
        unsafe { mikomai_fm_string_free(self.0.as_ptr()) }
    }
}

fn input(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| "Apple Foundation Models input contains an interior NUL".into())
}

impl Session {
    pub(crate) fn new(instructions: &str) -> Result<Self, String> {
        let instructions = input(instructions)?;
        let mut error = ptr::null_mut();
        // SAFETY: input and error out-pointer live through the synchronous call.
        let handle = unsafe { mikomai_fm_session_create(instructions.as_ptr(), &mut error) };
        let session = NonNull::new(handle).map(Self);
        if let Some(error) = NativeString::take(error) {
            // Even an unexpected handle+error result releases both allocations.
            return Err(error.to_rust()?);
        }
        session.ok_or_else(|| {
            "Apple Foundation Models session creation failed without an error".into()
        })
    }

    pub(crate) fn respond(&mut self, prompt: &str) -> Result<String, String> {
        let prompt = input(prompt)?;
        let mut error = ptr::null_mut();
        // SAFETY: handle is live, &mut self serializes calls; both inputs remain valid.
        let response =
            unsafe { mikomai_fm_session_respond(self.0.as_ptr(), prompt.as_ptr(), &mut error) };
        let response = NativeString::take(response);
        if let Some(error) = NativeString::take(error) {
            return Err(error.to_rust()?);
        }
        response
            .ok_or_else(|| "Apple Foundation Models returned no response or error".to_owned())?
            .to_rust()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: sole ownership, no outstanding respond call, destroyed once.
        unsafe { mikomai_fm_session_destroy(self.0.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interior_nul_is_rejected_before_native_creation() {
        assert!(Session::new("system\0instructions")
            .err()
            .unwrap()
            .contains("NUL"));
        assert!(input("prompt\0suffix").is_err());
        assert_eq!(input("日本語\n").unwrap().to_str().unwrap(), "日本語\n");
    }

    #[test]
    fn native_errors_are_owned_and_decoded_without_a_provisioned_model() {
        let invalid = [0xff_u8, 0];
        let mut error = ptr::null_mut();
        // SAFETY: NUL-terminated readable input and a valid out-pointer. This
        // invalid UTF-8 input is rejected before accessing the system model.
        let handle = unsafe { mikomai_fm_session_create(invalid.as_ptr().cast(), &mut error) };
        assert!(handle.is_null());
        let error = NativeString::take(error).expect("owned native error");
        assert!(error.to_rust().unwrap().contains("Invalid UTF-8"));
        drop(error); // Use the real Swift free function for this allocation.

        let prompt = input("hello").unwrap();
        let mut error = ptr::null_mut();
        // SAFETY: the ABI explicitly rejects a null handle without dereferencing.
        let response =
            unsafe { mikomai_fm_session_respond(ptr::null_mut(), prompt.as_ptr(), &mut error) };
        assert!(response.is_null());
        assert!(NativeString::take(error)
            .unwrap()
            .to_rust()
            .unwrap()
            .contains("Invalid session"));
    }
}
