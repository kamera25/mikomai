use std::fs::{self, OpenOptions, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_log::LogTracer;

struct SharedFileWriter {
    file: Arc<Mutex<File>>,
}

impl<'a> MakeWriter<'a> for SharedFileWriter {
    type Writer = SharedFileWriterLock<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFileWriterLock(self.file.lock().unwrap())
    }
}

struct SharedFileWriterLock<'a>(MutexGuard<'a, File>);

impl<'a> Write for SharedFileWriterLock<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Redirect standard log crate events to tracing, ignoring error if already set
    let _ = LogTracer::init();

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    let console_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_target(false)
        .with_file(true)
        .with_line_number(true);

    let file_layer = if let Some(dir) = get_app_data_dir() {
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("app.log");
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
            let file_writer = SharedFileWriter {
                file: Arc::new(Mutex::new(file)),
            };
            Some(fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false)
                .with_target(false)
                .with_file(true)
                .with_line_number(true))
        } else {
            None
        }
    } else {
        None
    };

    // Use try_init to avoid panicking if a global subscriber has already been set
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .try_init();

    Ok(())
}

fn get_app_data_dir() -> Option<PathBuf> {
    let bundle_identifier = "com.mikomai.agent";
    #[cfg(target_os = "macos")]
    {
        dirs::data_dir().map(|path| path.join(bundle_identifier))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir().map(|path| path.join(bundle_identifier))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs::config_dir().map(|path| path.join(bundle_identifier))
    }
}
