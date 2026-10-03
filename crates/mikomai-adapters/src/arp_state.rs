//! Graph-backed ARP infrastructure, shared by native callbacks and headless hosts.
use crate::portable_graph::{GraphDataKind, GraphIngestInput, PortableGraph};
use mikomai_core::{
    network::{
        arp::{self, CanonicalArpResult},
        arp_state::{ArpObservation, ArpStatePort},
    },
    port::PortFuture,
};
use serde_json::Value;

pub struct GraphArpState<'a> {
    pub graph: &'a PortableGraph,
    pub collect: &'a (dyn Fn(&str) -> Result<String, String> + Send + Sync),
    pub infer: fn(&str, &str) -> Result<String, String>,
    pub os_type: &'a str,
}
impl ArpStatePort for GraphArpState<'_> {
    fn fresh_observation<'a>(&'a self, device: &'a str) -> PortFuture<'a, Option<ArpObservation>> {
        Box::pin(async move { self.graph.fresh_arp_observation(device).await })
    }
    fn collect<'a>(&'a self, device: &'a str) -> PortFuture<'a, String> {
        Box::pin(async move { (self.collect)(device) })
    }
    fn canonicalize<'a>(
        &'a self,
        device: &'a str,
        observation: &'a ArpObservation,
    ) -> PortFuture<'a, CanonicalArpResult> {
        Box::pin(async move {
            arp::canonicalize(
                &observation.raw,
                device,
                self.os_type,
                observation.collected_at,
                self.infer,
            )
        })
    }
    fn store<'a>(
        &'a self,
        device: &'a str,
        observation: &'a ArpObservation,
        evidence: Option<Value>,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            self.graph.ingest(GraphIngestInput {
            source_id: "get_state.arp.read_through".into(), collected_at: observation.collected_at, device_name: device.into(), kind: GraphDataKind::Arp,
            raw: observation.raw.clone(), normalized: observation.canonical.as_ref().map(|table| {
                let ips = table["arp_table"].as_array().into_iter().flatten().map(|entry| serde_json::json!({"address":entry["ip_address"],"interface":entry["interface"]})).collect::<Vec<_>>();
                serde_json::json!({"ip_addresses":ips})
            }), canonical: observation.canonical.clone(), evidence,
            normalizer_version: "arp-constrained-index-v2".into(),
        }).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn selection(_: &str, grammar: &str) -> Result<String, String> {
        assert!(grammar.contains("ip ::= \"0\""));
        Ok(r#"{"is_arp_table":true,"entries":[{"ip_idx":0,"mac_idx":0,"interface_idx":0,"type":"dynamic","age_seconds":null}]}"#.into())
    }
    fn no_inference(_: &str, _: &str) -> Result<String, String> {
        panic!("cache hit must not infer")
    }
    async fn graph() -> (PortableGraph, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("mikomai-arp-state-{}", uuid::Uuid::new_v4()));
        (PortableGraph::initialize_at(&path).await.unwrap(), path)
    }
    #[tokio::test]
    async fn graph_miss_collects_once_then_hits_and_preserves_negative_lookup() {
        let (graph, path) = graph().await;
        let calls = AtomicUsize::new(0);
        let collector = |_: &str| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok("any-port 00:11:22:33:44:55 192.0.2.10 100".into())
        };
        let state = GraphArpState {
            graph: &graph,
            collect: &collector,
            infer: selection,
            os_type: "unknown",
        };
        let table = mikomai_core::network::arp_state::get_state(&state, "router")
            .await
            .unwrap();
        assert_eq!(table["arp_table"][0]["interface"], "any-port");
        let cached = GraphArpState {
            infer: no_inference,
            ..state
        };
        assert_eq!(
            mikomai_core::network::arp_state::get_state(&cached, "router")
                .await
                .unwrap(),
            table
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let answer = arp::mac_lookup_answer("router", "ff:ff:ff:ff:ff:ff", &table.to_string());
        assert!(answer.contains("存在しません"));
        let lookup = graph
            .find_endpoint(
                crate::portable_graph::EndpointLookup::IpByMac,
                "00:11:22:33:44:55",
                Some("router"),
            )
            .await
            .unwrap();
        assert_eq!(lookup["matches"][0]["ip_address"], "192.0.2.10");
        drop(graph);
        let _ = std::fs::remove_dir_all(path);
    }
    #[tokio::test]
    async fn fresh_raw_is_canonicalized_without_fetch_and_expired_data_is_refreshed() {
        let (graph, path) = graph().await;
        let collector = |_: &str| -> Result<String, String> { panic!("fresh raw must not fetch") };
        let state = GraphArpState {
            graph: &graph,
            collect: &collector,
            infer: selection,
            os_type: "unknown",
        };
        let observed_at = Utc::now() - Duration::minutes(1);
        state
            .store(
                "router",
                &ArpObservation {
                    raw: "192.0.2.10 00:11:22:33:44:55 novel0".into(),
                    canonical: None,
                    collected_at: observed_at,
                },
                None,
            )
            .await
            .unwrap();
        let table = mikomai_core::network::arp_state::get_state(&state, "router")
            .await
            .unwrap();
        assert_eq!(
            graph
                .fresh_arp_observation("router")
                .await
                .unwrap()
                .unwrap()
                .collected_at,
            observed_at
        );
        assert_eq!(
            mikomai_core::network::arp_state::get_state(
                &GraphArpState {
                    infer: no_inference,
                    ..state
                },
                "router"
            )
            .await
            .unwrap(),
            table
        );
        state
            .store(
                "old-router",
                &ArpObservation {
                    raw: String::new(),
                    canonical: Some(table.clone()),
                    collected_at: Utc::now() - Duration::minutes(21),
                },
                None,
            )
            .await
            .unwrap();
        assert!(graph
            .fresh_arp_observation("old-router")
            .await
            .unwrap()
            .is_none());
        let calls = AtomicUsize::new(0);
        let collector = |_: &str| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok("novel0 192.0.2.20 00:11:22:33:44:66 20".into())
        };
        let state = GraphArpState {
            collect: &collector,
            ..state
        };
        let updated = mikomai_core::network::arp_state::get_state(&state, "old-router")
            .await
            .unwrap();
        assert_eq!(updated["arp_table"][0]["ip_address"], "192.0.2.20");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        drop(graph);
        let _ = std::fs::remove_dir_all(path);
    }
    #[tokio::test]
    async fn inference_failure_keeps_raw_for_retry_and_never_publishes_a_partial_table() {
        let (graph, path) = graph().await;
        let calls = AtomicUsize::new(0);
        let collector = |_: &str| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok("novel0 192.0.2.10 00:11:22:33:44:55 100".into())
        };
        let state = GraphArpState {
            graph: &graph,
            collect: &collector,
            infer: |_, _| Err("inference failed".into()),
            os_type: "unknown",
        };
        assert!(
            mikomai_core::network::arp_state::get_state(&state, "router")
                .await
                .is_err()
        );
        assert!(graph
            .fresh_arp_observation("router")
            .await
            .unwrap()
            .unwrap()
            .canonical
            .is_none());
        let retry = GraphArpState {
            infer: selection,
            ..state
        };
        assert!(
            mikomai_core::network::arp_state::get_state(&retry, "router")
                .await
                .is_ok()
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        drop(graph);
        let _ = std::fs::remove_dir_all(path);
    }

    /// Run explicitly with MIKOMAI_ARP_TEST_MODEL; uses actual grammar sampling,
    /// canonical validation and persisted graph state (no fake inference).
    #[tokio::test]
    #[ignore = "requires an explicitly configured local GGUF model"]
    async fn real_llm_canonicalization_and_graph_cache() {
        let model = std::env::var("MIKOMAI_ARP_TEST_MODEL").expect("set MIKOMAI_ARP_TEST_MODEL");
        crate::local_llama::load(std::path::Path::new(&model)).unwrap();
        crate::local_llama::set_params(0.0, 1.0, 8192, 2048).unwrap();
        let path = std::env::var_os("MIKOMAI_ARP_TEST_DB")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("mikomai-real-arp-{}", uuid::Uuid::new_v4()))
            });
        let graph = PortableGraph::initialize_at(&path).await.unwrap();
        for (device, raw, ip, mac, interface) in [
            ("localhost", "カウント数: 1\nインタフェース IPアドレス MACアドレス TTL(秒)\nLAN1(port1) 192.0.2.10 44:55:66:b3:37:22 1194", "192.0.2.10", "44:55:66:b3:37:22", "LAN1(port1)"),
            ("cisco", "Protocol Address Age (min) Hardware Addr Type Interface\nInternet 192.0.2.11 2 0011.2233.4455 ARPA Gi1/0/1", "192.0.2.11", "00:11:22:33:44:55", "Gi1/0/1"),
            ("novel", "Neighbor cache\nMAC=port-independent ordering\nnovel-port aa:bb:cc:dd:ee:ff 192.0.2.12 dynamic 45", "192.0.2.12", "aa:bb:cc:dd:ee:ff", "novel-port"),
        ] {
            let collector = |_: &str| Ok(raw.to_string());
            let state = GraphArpState { graph: &graph, collect: &collector, infer: crate::local_llama::infer_constrained, os_type: "unknown" };
            let table = mikomai_core::network::arp_state::get_state(&state, device).await.unwrap();
            assert_eq!(table["arp_table"][0]["ip_address"], ip);
            assert_eq!(table["arp_table"][0]["mac_address"], mac);
            assert_eq!(table["arp_table"][0]["interface"], interface);
            let collector = |_: &str| -> Result<String, String> { panic!("cached graph must not collect") };
            let cached = GraphArpState { collect: &collector, infer: no_inference, ..state };
            assert_eq!(mikomai_core::network::arp_state::get_state(&cached, device).await.unwrap(), table);
        }
    }
}
