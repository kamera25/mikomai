//! Transparent graph read-through: collectors and canonicalizers are infrastructure,
//! and the agent sees only a validated canonical ARP table from get_state.
use super::arp::{validate_canonical_table, CanonicalArpResult};
use crate::port::PortFuture;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct ArpObservation {
    pub raw: String,
    pub canonical: Option<Value>,
    pub collected_at: DateTime<Utc>,
}

pub trait ArpStatePort: Send + Sync {
    fn fresh_observation<'a>(&'a self, device: &'a str) -> PortFuture<'a, Option<ArpObservation>>;
    fn collect<'a>(&'a self, device: &'a str) -> PortFuture<'a, String>;
    fn canonicalize<'a>(
        &'a self,
        device: &'a str,
        observation: &'a ArpObservation,
    ) -> PortFuture<'a, CanonicalArpResult>;
    fn store<'a>(
        &'a self,
        device: &'a str,
        observation: &'a ArpObservation,
        evidence: Option<Value>,
    ) -> PortFuture<'a, ()>;
}

pub async fn get_state(port: &impl ArpStatePort, device: &str) -> Result<Value, String> {
    let mut observation = match port.fresh_observation(device).await? {
        Some(observation) => observation,
        None => {
            let raw = port.collect(device).await?;
            if raw.trim().is_empty() {
                return Err("ARP取得結果が空のため、有無を判定できません。".into());
            }
            let observation = ArpObservation {
                raw,
                canonical: None,
                collected_at: Utc::now(),
            };
            port.store(device, &observation, None).await?;
            observation
        }
    };
    if let Some(table) = &observation.canonical {
        // Invalid legacy entries cannot become authoritative evidence of absence.
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
        &observation,
        Some(serde_json::to_value(result.evidence).map_err(|e| e.to_string())?),
    )
    .await?;
    Ok(table)
}
