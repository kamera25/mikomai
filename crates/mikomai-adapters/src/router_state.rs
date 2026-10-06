//! ARP-style graph read-through for all catalogued native router resources.
use crate::{
    portable_graph::{GraphDataKind, GraphIngestInput, PortableGraph, GRAPH_TTL_MINUTES},
    router_canonicalization as canonical, router_schema,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct RouterObservation {
    pub raw: String,
    pub canonical: Option<Value>,
    pub collected_at: DateTime<Utc>,
    pub source_id: String,
}

pub struct GraphRouterState<'a> {
    pub graph: &'a PortableGraph,
    pub collect: &'a (dyn Fn(&str, &str) -> Result<String, String> + Send + Sync),
    pub infer: fn(&str, &str) -> Result<String, String>,
    pub os_type: &'a str,
}

impl GraphRouterState<'_> {
    async fn store(
        &self,
        device: &str,
        table: &str,
        observation: &RouterObservation,
        normalized: Option<Value>,
        evidence: Option<Value>,
    ) -> Result<(), String> {
        self.graph
            .ingest(GraphIngestInput {
                source_id: observation.source_id.clone(),
                collected_at: observation.collected_at,
                device_name: device.into(),
                kind: serde_json::from_value::<GraphDataKind>(Value::String(table.into()))
                    .map_err(|e| e.to_string())?,
                raw: observation.raw.clone(),
                normalized,
                canonical: observation.canonical.clone(),
                evidence,
                normalizer_version: canonical::VERSION.into(),
            })
            .await
    }

    pub async fn get_state(
        &self,
        device: &str,
        table: &str,
        refresh: bool,
    ) -> Result<Value, String> {
        router_schema::resource_schema(table)?;
        if device.trim().is_empty() {
            return Err("Router observation requires a device".into());
        }
        let latest = if refresh {
            None
        } else {
            self.graph.latest_router_observation(device, table).await?
        };
        let latest = latest.filter(|o| {
            let age = Utc::now() - o.collected_at;
            age >= Duration::zero() && age <= Duration::minutes(GRAPH_TTL_MINUTES)
        });
        let mut observation = match latest {
            Some(observation) => observation,
            None => {
                // Persist failed/empty refreshes too, so older successes cannot revive.
                let result = (self.collect)(device, table);
                let observation = RouterObservation {
                    raw: result.as_ref().cloned().unwrap_or_default(),
                    canonical: None,
                    collected_at: Utc::now(),
                    source_id: format!("get_state.{table}"),
                };
                self.store(device, table, &observation, None, None).await?;
                let raw = result?;
                if raw.trim().is_empty() {
                    return Err(format!("{table} collection is empty; state is unknown"));
                }
                observation
            }
        };
        if let Some(value) = &observation.canonical {
            if canonical::validate_canonical(value, table, device, observation.collected_at).is_ok()
            {
                return Ok(value.clone());
            }
        }
        let result = canonical::canonicalize(
            &observation.raw,
            table,
            device,
            self.os_type,
            observation.collected_at,
            self.infer,
        )?;
        observation.canonical = Some(result.canonical.clone());
        self.store(
            device,
            table,
            &observation,
            Some(result.normalized),
            Some(result.evidence),
        )
        .await?;
        Ok(result.canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn infer(prompt: &str, _: &str) -> Result<String, String> {
        let raw = prompt
            .split("\nRaw:\n")
            .nth(1)
            .unwrap()
            .split("\nPrevious selection")
            .next()
            .unwrap();
        let candidates = canonical::extract(raw)?;
        let index = candidates
            .iter()
            .position(|c| c.value == json!("192.0.2.53"))
            .unwrap();
        Ok(json!({"complete":true,"empty_line":null,"entries":[{"start_line":1,"end_line":1,"address":index,"port":null,"source_address":null,"vrf":null}]}).to_string())
    }
    fn no_inference(_: &str, _: &str) -> Result<String, String> {
        panic!("cache hit must not infer")
    }

    #[tokio::test]
    async fn raw_retry_cache_refresh_and_failure_preserve_timestamp_and_facts() {
        let path =
            std::env::temp_dir().join(format!("mikomai-router-state-{}", uuid::Uuid::new_v4()));
        let graph = PortableGraph::initialize_at(&path).await.unwrap();
        let calls = AtomicUsize::new(0);
        let collector = |_: &str, table: &str| {
            assert_eq!(table, "dns_server");
            calls.fetch_add(1, Ordering::Relaxed);
            Ok("DNS server 192.0.2.53".into())
        };
        let state = GraphRouterState {
            graph: &graph,
            collect: &collector,
            infer: |_, _| Err("model unavailable".into()),
            os_type: "unknown",
        };
        assert!(state.get_state("r", "dns_server", false).await.is_err());
        let observation = graph
            .latest_router_observation("r", "dns_server")
            .await
            .unwrap()
            .unwrap();
        assert!(observation.canonical.is_none());
        let state = GraphRouterState { infer, ..state };
        let canonical = state.get_state("r", "dns_server", false).await.unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            graph
                .latest_router_observation("r", "dns_server")
                .await
                .unwrap()
                .unwrap()
                .collected_at,
            observation.collected_at
        );
        assert_eq!(
            graph.router_facts("dns_server", "r").await.unwrap()[0]["address"],
            "192.0.2.53"
        );
        let cached = GraphRouterState {
            infer: no_inference,
            ..state
        };
        assert_eq!(
            cached.get_state("r", "dns_server", false).await.unwrap(),
            canonical
        );
        let failure = |_: &str, _: &str| Err("collection failed".into());
        let failed = GraphRouterState {
            collect: &failure,
            ..state
        };
        assert!(failed.get_state("r", "dns_server", true).await.is_err());
        assert!(failed.get_state("r", "dns_server", false).await.is_err());
        assert!(graph
            .latest_router_observation("r", "dns_server")
            .await
            .unwrap()
            .unwrap()
            .canonical
            .is_none());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        state.get_state("r", "dns_server", true).await.unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        let mut old = graph
            .latest_router_observation("r", "dns_server")
            .await
            .unwrap()
            .unwrap();
        old.collected_at = Utc::now() - Duration::minutes(21);
        state
            .store("old-router", "dns_server", &old, None, None)
            .await
            .unwrap();
        state
            .get_state("old-router", "dns_server", false)
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        drop(graph);
        let _ = std::fs::remove_dir_all(path);
    }
}
