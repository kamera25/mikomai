//! Vendor-independent graph read-through for LLM-canonicalized interface state.
use super::interface::{validate_canonical_table, CanonicalInterfaceResult};
use crate::port::PortFuture;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug,serde::Serialize,serde::Deserialize)]
pub struct InterfaceObservation {
    pub raw: String,
    pub canonical: Option<Value>,
    pub collected_at: DateTime<Utc>,
}

pub trait InterfaceStatePort: Send + Sync {
    fn latest_observation<'a>(
        &'a self,
        device: &'a str,
        scope: &'a str,
    ) -> PortFuture<'a, Option<InterfaceObservation>>;
    fn collect<'a>(&'a self, device: &'a str) -> PortFuture<'a, String>;
    fn canonicalize<'a>(
        &'a self,
        device: &'a str,
        observation: &'a InterfaceObservation,
    ) -> PortFuture<'a, CanonicalInterfaceResult>;
    fn store<'a>(
        &'a self,
        device: &'a str,
        scope: &'a str,
        observation: &'a InterfaceObservation,
        evidence: Option<Value>,
    ) -> PortFuture<'a, ()>;
}

pub async fn get_state(
    port: &impl InterfaceStatePort,
    device: &str,
    scope: &str,
    refresh: bool,
) -> Result<Value, String> {
    let cached = if refresh {
        None
    } else {
        port.latest_observation(device, scope)
            .await?
            .filter(|observation| cache_is_fresh(observation.collected_at, Utc::now()))
    };
    let mut observation = match cached {
        Some(observation) => observation,
        None => {
            let collected = port.collect(device).await;
            let raw = collected.as_ref().cloned().unwrap_or_default();
            let observation = InterfaceObservation {
                raw,
                canonical: None,
                collected_at: Utc::now(),
            };
            port.store(device, scope, &observation, None).await?;
            collected?;
            if observation.raw.trim().is_empty() {
                return Err("インターフェース取得結果が空です".into());
            }
            observation
        }
    };
    if let Some(table) = &observation.canonical {
        if validate_canonical_table(table, device).is_ok() {
            return Ok(table.clone());
        }
    }
    let result = port.canonicalize(device, &observation).await?;
    let table = serde_json::to_value(result.table).map_err(|e| e.to_string())?;
    validate_canonical_table(&table, device)?;
    observation.canonical = Some(table.clone());
    port.store(
        device,
        scope,
        &observation,
        Some(serde_json::to_value(result.evidence).map_err(|e| e.to_string())?),
    )
    .await?;
    // Calculate against saved graph state, not an unsaved LLM response.
    let saved = port
        .latest_observation(device, scope)
        .await?
        .and_then(|o| o.canonical)
        .ok_or("Canonical観測をGraphから再取得できません")?;
    validate_canonical_table(&saved, device)?;
    if saved != table {
        return Err("Graphの観測が更新されたため、再確認が必要です".into());
    }
    Ok(saved)
}

fn cache_is_fresh(collected_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let age = now - collected_at;
    age >= chrono::Duration::zero() && age <= chrono::Duration::seconds(60)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_expiry_is_applied_to_reuse_only() {
        let now = Utc::now();
        assert!(cache_is_fresh(now - chrono::Duration::seconds(60), now));
        assert!(!cache_is_fresh(now - chrono::Duration::seconds(61), now));
        assert!(!cache_is_fresh(now + chrono::Duration::seconds(1), now));
    }
}
