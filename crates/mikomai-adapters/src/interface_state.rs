//! Graph storage and infrastructure inference for portable interface state.
use crate::portable_graph::{GraphDataKind, GraphIngestInput, PortableGraph};
use mikomai_core::{
    network::{
        interface::{self, CanonicalInterfaceResult},
        interface_state::{InterfaceObservation, InterfaceStatePort},
    },
    port::PortFuture,
};
use serde_json::{json, Value};

pub struct GraphInterfaceState<'a> {
    pub graph: &'a PortableGraph,
    pub collect: &'a (dyn Fn(&str) -> Result<String, String> + Send + Sync),
    pub infer: fn(&str, &str) -> Result<String, String>,
    pub os_type: &'a str,
}
impl InterfaceStatePort for GraphInterfaceState<'_> {
    fn latest_observation<'a>(
        &'a self,
        device: &'a str,
        scope: &'a str,
    ) -> PortFuture<'a, Option<InterfaceObservation>> {
        Box::pin(async move { self.graph.latest_interface_observation(device, scope).await })
    }
    fn collect<'a>(&'a self, device: &'a str) -> PortFuture<'a, String> {
        Box::pin(async move { (self.collect)(device) })
    }
    fn canonicalize<'a>(
        &'a self,
        device: &'a str,
        observation: &'a InterfaceObservation,
    ) -> PortFuture<'a, CanonicalInterfaceResult> {
        Box::pin(async move {
            interface::canonicalize(
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
        scope: &'a str,
        observation: &'a InterfaceObservation,
        evidence: Option<Value>,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let normalized = observation.canonical.as_ref().map(|table| {
                let ips = table["interfaces"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|entry| {
                        entry["ipv4_addresses"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(move |ip| json!({"address":ip,"interface":entry["name"]}))
                    })
                    .collect::<Vec<_>>();
                json!({"interfaces":table["interfaces"],"ip_addresses":ips})
            });
            self.graph
                .ingest(GraphIngestInput {
                    source_id: format!("get_state.interfaces:{scope}"),
                    collected_at: observation.collected_at,
                    device_name: device.into(),
                    kind: GraphDataKind::Interfaces,
                    raw: observation.raw.clone(),
                    normalized,
                    canonical: observation.canonical.clone(),
                    evidence,
                    normalizer_version: "interface-constrained-index-v1".into(),
                })
                .await
        })
    }
}
