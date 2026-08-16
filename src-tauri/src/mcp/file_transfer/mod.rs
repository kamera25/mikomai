use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileTransferResult
{
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<std::path::PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_transferred: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl From<FileTransferResult> for crate::network::CommandResult
{
    fn from(res: FileTransferResult) -> Self
    {
        Self {
            success: res.success,
            output: res.output,
            saved_path: res.file_path,
            is_cached: None,
            cache_time: None,
        }
    }
}

pub mod ftp;
pub mod tftp;
