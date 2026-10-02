//! macOS-only, stateless Apple Foundation Models integration via Apple's `fm`.
//! The workspace excludes this crate; consumers use macOS target dependencies.
//! Defensive cfg also removes its implementation on other targets.
#![cfg(target_os = "macos")]

use mikomai_llm::{
    InferenceCapabilities, InferencePort, ModelAvailability, PortFuture, TokenLimits,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Conservative on-device budget; kept here rather than in callers or core.
pub const CONTEXT_WINDOW: u32 = 4096;
const RESPONSE_RESERVE: u32 = 1024;

/// Replaceable transport boundary for a future native integration.
/// Token counts must include conversation framing and any transport instructions.
pub trait AppleTransport: Send + Sync {
    fn availability(&self) -> ModelAvailability;
    fn count_tokens(&self, prompt: &str) -> Result<u32, String>;
    fn generate(&self, prompt: &str) -> Result<String, String>;
}

pub struct AppleInference<T = FmCli> {
    transport: T,
}

impl Default for AppleInference<FmCli> {
    fn default() -> Self {
        Self::new(FmCli::default())
    }
}

impl<T: AppleTransport> AppleInference<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    fn generate(&self, prompt: &str) -> Result<String, String> {
        if let ModelAvailability::Unavailable { reason } = self.availability() {
            return Err(format!("Apple Foundation Models unavailable: {reason}"));
        }
        let tokens = self.transport.count_tokens(prompt)?;
        if tokens > CONTEXT_WINDOW - RESPONSE_RESERVE {
            return Err(format!(
                "Apple prompt uses {tokens} tokens; at most {} are allowed with response headroom. Shorten the prompt.",
                CONTEXT_WINDOW - RESPONSE_RESERVE
            ));
        }
        let response = self.transport.generate(prompt)?;
        if response.trim().is_empty() {
            return Err("Apple Foundation Models returned an empty response".into());
        }
        Ok(response.trim().to_owned())
    }
}

impl<T: AppleTransport> InferencePort for AppleInference<T> {
    fn complete<'a>(&'a self, prompt: &'a str) -> PortFuture<'a, String> {
        Box::pin(async move { self.generate(prompt) })
    }

    fn capabilities(&self) -> InferenceCapabilities {
        InferenceCapabilities {
            // This first transport exposes text only. fm's schema format and
            // built-in tools are not the application's structured/tool API.
            token_limits: TokenLimits {
                context_window: Some(CONTEXT_WINDOW),
                // fm respond has no output-token cap option.
                max_output_tokens: None,
            },
            ..InferenceCapabilities::default()
        }
    }

    fn availability(&self) -> ModelAvailability {
        self.transport.availability()
    }
}

/// Apple's system CLI, not third-party commands also named fm. It must already
/// be installed and provisioned; this backend never installs or accepts terms.
pub struct FmCli {
    executable: PathBuf,
}

impl Default for FmCli {
    fn default() -> Self {
        Self::new("/usr/bin/fm")
    }
}

impl FmCli {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    fn run(&self, args: &[&str], prompt: Option<&str>) -> Result<String, String> {
        let mut child = Command::new(&self.executable)
            .args(args)
            .env("NO_COLOR", "1")
            .stdin(if prompt.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot start fm: {e}"))?;
        // Drain stdout/stderr while writing stdin to avoid pipe deadlocks.
        // Closing stdin signals EOF; no prompt text is interpreted as options.
        let writer = prompt.map(|prompt| {
            let mut stdin = child.stdin.take().expect("piped stdin");
            let input = prompt.as_bytes().to_vec();
            std::thread::spawn(move || stdin.write_all(&input))
        });
        let output = child
            .wait_with_output()
            .map_err(|e| format!("fm failed: {e}"))?;
        let write_result = writer.map(|writer| writer.join());
        if !output.status.success() {
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "fm exited with {}: {} {}",
                output.status,
                diagnostic.trim(),
                stdout.trim()
            ));
        }
        if let Some(result) = write_result {
            result
                .map_err(|_| "fm input writer failed".to_string())?
                .map_err(|e| format!("Cannot send prompt to fm: {e}"))?;
        }
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from fm: {e}"))
    }
}

impl AppleTransport for FmCli {
    fn availability(&self) -> ModelAvailability {
        match self.run(&["available", "--model", "system"], None) {
            // Do not equate process success or executable presence with model
            // availability. Unknown CLI output is treated as unavailable.
            Ok(output) if output.trim() == "System model available" => ModelAvailability::Available,
            Ok(output) => ModelAvailability::Unavailable {
                reason: format!("fm did not confirm model availability: {}", output.trim()),
            },
            Err(reason) => ModelAvailability::Unavailable { reason },
        }
    }

    fn count_tokens(&self, prompt: &str) -> Result<u32, String> {
        self.run(&["count-tokens", "--quiet"], Some(prompt))?
            .trim()
            .parse()
            .map_err(|e| format!("Invalid fm token count: {e}"))
    }

    fn generate(&self, prompt: &str) -> Result<String, String> {
        self.run(
            &["respond", "--model", "system", "--no-stream"],
            Some(prompt),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct Fake {
        available: bool,
        tokens: u32,
        generated: AtomicBool,
        response: &'static str,
    }
    impl AppleTransport for Fake {
        fn availability(&self) -> ModelAvailability {
            if self.available {
                ModelAvailability::Available
            } else {
                ModelAvailability::Unavailable {
                    reason: "model assets not ready".into(),
                }
            }
        }
        fn count_tokens(&self, _: &str) -> Result<u32, String> {
            Ok(self.tokens)
        }
        fn generate(&self, _: &str) -> Result<String, String> {
            self.generated.store(true, Ordering::Relaxed);
            Ok(self.response.into())
        }
    }
    fn backend(available: bool, tokens: u32, response: &'static str) -> AppleInference<Fake> {
        AppleInference::new(Fake {
            available,
            tokens,
            generated: AtomicBool::new(false),
            response,
        })
    }
    #[test]
    fn interchangeable_core_port_generates_text() {
        let backend = backend(true, 3072, "  こんにちは  ");
        let port: &dyn InferencePort = &backend;
        assert_eq!(
            futures_lite::future::block_on(port.complete("挨拶してください")).unwrap(),
            "こんにちは"
        );
        assert_eq!(port.capabilities().token_limits.context_window, Some(4096));
        assert!(!port.capabilities().structured_output);
        assert!(!port.capabilities().tool_calling);
    }
    #[test]
    fn unavailable_model_does_not_generate_even_on_macos() {
        let backend = backend(false, 10, "unexpected");
        assert!(backend
            .generate("hello")
            .unwrap_err()
            .contains("assets not ready"));
        assert!(!backend.transport.generated.load(Ordering::Relaxed));
    }
    #[test]
    fn oversized_context_is_rejected_without_truncation() {
        let backend = backend(true, 3073, "unexpected");
        assert!(backend
            .generate("hello")
            .unwrap_err()
            .contains("3073 tokens"));
        assert!(!backend.transport.generated.load(Ordering::Relaxed));
    }
    #[test]
    fn empty_output_is_an_error() {
        assert!(backend(true, 1, " \n")
            .generate("hello")
            .unwrap_err()
            .contains("empty response"));
    }
    #[test]
    fn missing_cli_is_runtime_unavailability() {
        assert!(matches!(
            FmCli::new("/nonexistent/mikomai-fm").availability(),
            ModelAvailability::Unavailable { .. }
        ));
    }

    struct Script(PathBuf);
    impl Script {
        fn new(body: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let directory = std::env::temp_dir().join(format!(
                "mikomai-fm-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&directory).unwrap();
            let path = directory.join("fm");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }
    impl Drop for Script {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn cli_transport_preserves_stdin_and_selects_system_model() {
        let script = Script::new(
            r#"
case "$*" in
  'available --model system') echo 'System model available';;
  'count-tokens --quiet') cat >/dev/null; echo 12;;
  'respond --model system --no-stream') cat;;
  *) echo 'unexpected CLI arguments' >&2; exit 2;;
esac
"#,
        );
        let backend = AppleInference::new(FmCli::new(&script.0));
        let prompt = "--help\n$(this must remain literal)\n日本語";
        assert_eq!(backend.generate(prompt).unwrap(), prompt);
    }

    #[test]
    fn zero_exit_without_model_confirmation_is_unavailable() {
        let script = Script::new("echo 'System model unavailable: Apple Intelligence disabled'");
        assert!(matches!(
            FmCli::new(&script.0).availability(),
            ModelAvailability::Unavailable { .. }
        ));
    }

    #[test]
    fn cli_errors_and_invalid_token_counts_are_not_generation_success() {
        let error = Script::new("echo 'model assets not ready' >&2; exit 1");
        assert!(FmCli::new(&error.0)
            .generate("hello")
            .unwrap_err()
            .contains("model assets not ready"));
        let invalid = Script::new("cat >/dev/null; echo 'not a number'");
        assert!(FmCli::new(&invalid.0)
            .count_tokens("hello")
            .unwrap_err()
            .contains("Invalid fm token count"));
    }
}
