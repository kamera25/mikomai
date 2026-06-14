use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use log::{Record, Level, Metadata, SetLoggerError, LevelFilter};
use chrono::Local;

struct SimpleLogger {
    log_file_path: OnceLock<Option<PathBuf>>,
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level_color = match record.level() {
                Level::Error => "\x1b[31m", // Red
                Level::Warn => "\x1b[33m",  // Yellow
                Level::Info => "\x1b[32m",  // Green
                Level::Debug => "\x1b[36m", // Cyan
                Level::Trace => "\x1b[35m", // Magenta
            };
            let reset_color = "\x1b[0m";

            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let file = record.file().unwrap_or("<unknown>");
            let line = record.line().unwrap_or(0);
            
            // Console output
            println!(
                "{} {}{:<5}{} [{}:{}] {}",
                timestamp,
                level_color,
                record.level(),
                reset_color,
                file,
                line,
                record.args()
            );

            // File output
            if let Some(Some(ref path)) = self.log_file_path.get() {
                let log_line = format!(
                    "{} {:<5} [{}:{}] {}\n",
                    timestamp,
                    record.level(),
                    file,
                    line,
                    record.args()
                );
                if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                    let _ = f.write_all(log_line.as_bytes());
                }
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger {
    log_file_path: OnceLock::new(),
};

pub fn init() -> Result<(), SetLoggerError> {
    let log_file_path = get_app_data_dir().map(|dir| {
        let _ = fs::create_dir_all(&dir);
        dir.join("app.log")
    });

    let _ = LOGGER.log_file_path.set(log_file_path);

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Debug))
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
