#[derive(Debug, thiserror::Error)]
pub enum MikomaiError {
    #[error("Settings error: {0}")]
    Settings(#[from] crate::settings::SettingsError),

    #[error("LLM error: {0}")]
    Llm(#[from] crate::llm::LlmError),

    #[error("Crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),

    #[error("History error: {0}")]
    History(#[from] crate::history::HistoryError),

    #[error("Connection error: {0}")]
    Connection(#[from] crate::connections::ConnectionError),

    #[error("Scheduled task error: {0}")]
    ScheduledTask(#[from] crate::scheduled_tasks::ScheduledTaskError),

    #[error("Network error: {0}")]
    Network(#[from] crate::network::NetworkError),

    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct TauriError(pub MikomaiError);

impl std::fmt::Display for TauriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for TauriError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<E> From<E> for TauriError
where
    E: Into<MikomaiError>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
