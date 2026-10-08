//! Stored-state service. Resource identities come from the existing catalog.
use crate::{portable_graph::PortableGraph, router_schema};
use mikomai_core::network::state::{
    self, DiffStateInput, QueryStateInput, ResourceShape, StateSnapshot,
};
use serde_json::Value;

pub fn resource_shape(resource: &str) -> Result<ResourceShape, String> {
    let (collection, identity, fields, unordered) = match resource {
        "interfaces" => (
            "interfaces",
            vec!["name"],
            vec!["name", "status", "ipv4_addresses", "prefix_len"],
            vec!["ipv4_addresses"],
        ),
        "arp" => (
            "arp_table",
            vec!["ip_address", "interface"],
            vec![
                "ip_address",
                "mac_address",
                "type",
                "interface",
                "age_seconds",
            ],
            vec![],
        ),
        other => {
            let schema = router_schema::resource_schema(other)?;
            return Ok(ResourceShape {
                collection: other.into(),
                identity: schema.identity.clone(),
                fields: schema.fields.iter().map(|f| f.name.clone()).collect(),
                unordered_fields: vec![],
            });
        }
    };
    Ok(ResourceShape {
        collection: collection.into(),
        identity: identity.into_iter().map(str::to_owned).collect(),
        fields: fields.into_iter().map(str::to_owned).collect(),
        unordered_fields: unordered.into_iter().map(str::to_owned).collect(),
    })
}
fn bounded_output(value: Value) -> Result<Value, String> {
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("State output exceeds 1 MiB; narrow fields/filter or reduce limit".into());
    }
    Ok(value)
}
pub async fn execute(graph: &PortableGraph, tool: &str, args: &Value) -> Result<Value, String> {
    match tool {
        "query_state" => {
            let input: QueryStateInput =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            state::validate_selector(
                &input.device,
                &input.resource,
                &input.scope,
                &[&input.snapshot_id],
                input.limit,
            )?;
            let shape = resource_shape(&input.resource)?;
            let snapshot = load(
                graph,
                &input.snapshot_id,
                &input.device,
                &input.resource,
                &input.scope,
            )
            .await?;
            bounded_output(
                serde_json::to_value(state::query_state(&snapshot, &input, &shape)?)
                    .map_err(|e| e.to_string())?,
            )
        }
        "diff_state" => {
            let input: DiffStateInput =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            state::validate_selector(
                &input.device,
                &input.resource,
                &input.scope,
                &[&input.before, &input.after],
                input.limit,
            )?;
            let shape = resource_shape(&input.resource)?;
            let before = load(
                graph,
                &input.before,
                &input.device,
                &input.resource,
                &input.scope,
            )
            .await?;
            let after = if input.before == input.after {
                before.clone()
            } else {
                load(
                    graph,
                    &input.after,
                    &input.device,
                    &input.resource,
                    &input.scope,
                )
                .await?
            };
            bounded_output(
                serde_json::to_value(state::diff_state(&before, &after, &input, &shape)?)
                    .map_err(|e| e.to_string())?,
            )
        }
        _ => Err(format!("Unsupported stored-state tool: {tool}")),
    }
}
async fn load(
    graph: &PortableGraph,
    id: &str,
    device: &str,
    resource: &str,
    scope: &str,
) -> Result<StateSnapshot, String> {
    graph
        .state_snapshot(id, device, resource, scope)
        .await?
        .ok_or_else(|| format!("State snapshot not found: {id} ({device}/{resource}/{scope})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portable_graph::{GraphDataKind, GraphIngestInput};
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    async fn graph() -> PortableGraph {
        PortableGraph::initialize_at(
            &std::env::temp_dir().join(format!("mikomai-stored-state-{}", uuid::Uuid::new_v4())),
        )
        .await
        .unwrap()
    }
    async fn ingest(graph: &PortableGraph, second: u32, scope: &str, canonical: Option<Value>) {
        graph
            .ingest(GraphIngestInput {
                source_id: format!("get_state.interfaces:{scope}"),
                device_name: "R1".into(),
                kind: GraphDataKind::Interfaces,
                collected_at: Utc.with_ymd_and_hms(2026, 10, 8, 0, 0, second).unwrap(),
                raw: "fixture".into(),
                canonical,
                normalized: None,
                evidence: None,
                normalizer_version: "interface-constrained-index-v1".into(),
            })
            .await
            .unwrap();
    }
    fn table(status: &str) -> Value {
        json!({"version":"1.0","metadata":{"source_device":"R1","generated_at":"2026-10-08T00:00:00Z","os_type":"fixture"},"interfaces":[{"name":"eth1","status":status,"ipv4_addresses":[],"prefix_len":null}]})
    }
    fn request(id: &str) -> Value {
        json!({"snapshot_id":id,"device":"R1","resource":"interfaces"})
    }
    #[tokio::test]
    async fn historical_snapshot_latest_diff_and_unknown_selectors() {
        let graph = graph().await;
        ingest(&graph, 0, "all", Some(table("up"))).await;
        let first = execute(&graph, "query_state", &request("latest"))
            .await
            .unwrap();
        let id = first["snapshot"]["snapshot_id"].as_str().unwrap();
        assert!(!id.is_empty());
        assert_eq!(first["availability"], "complete");
        ingest(&graph, 1, "all", Some(table("down"))).await;
        let old = execute(&graph, "query_state", &request(id)).await.unwrap();
        assert_eq!(old["results"][0]["status"], "up");
        let changes = execute(
            &graph,
            "diff_state",
            &json!({"before":id,"after":"latest","device":"R1","resource":"interfaces"}),
        )
        .await
        .unwrap();
        assert_eq!(changes["changes"][0]["before"], "up");
        assert_eq!(changes["changes"][0]["after"], "down");
        assert!(execute(&graph, "query_state", &request("nonexistent"))
            .await
            .unwrap_err()
            .contains("not found"));
        assert!(execute(
            &graph,
            "diff_state",
            &json!({"before":"nonexistent","after":"latest","device":"R1","resource":"interfaces"})
        )
        .await
        .is_err());
        let mut wrong = request(id);
        wrong["device"] = json!("R2");
        assert!(execute(&graph, "query_state", &wrong).await.is_err());
    }
    #[tokio::test]
    async fn failed_latest_never_resurrects_success_and_scopes_stay_separate() {
        let graph = graph().await;
        ingest(&graph, 0, "all", Some(table("up"))).await;
        ingest(&graph, 1, "eth1", Some(table("down"))).await;
        let all = execute(&graph, "query_state", &request("latest"))
            .await
            .unwrap();
        assert_eq!(all["results"][0]["status"], "up");
        ingest(&graph, 2, "all", None).await;
        let result = execute(&graph, "query_state", &request("latest"))
            .await
            .unwrap();
        assert_eq!(result["availability"], "unavailable");
        assert_eq!(result["snapshot"]["complete"], false);
        assert_eq!(result["results"], json!([]));
        assert!(execute(&graph,"diff_state",&json!({"before":all["snapshot"]["snapshot_id"],"after":"latest","device":"R1","resource":"interfaces"})).await.unwrap_err().contains("incomplete"));
    }
    #[tokio::test]
    async fn failed_interface_collection_is_saved_and_broker_can_read_it() {
        use mikomai_core::network::interface_state::get_state;
        let root =
            std::env::temp_dir().join(format!("mikomai-state-broker-{}", uuid::Uuid::new_v4()));
        let graph = PortableGraph::initialize_at(&root).await.unwrap();
        ingest(&graph, 0, "all", Some(table("up"))).await;
        let state = crate::interface_state::GraphInterfaceState {
            graph: &graph,
            collect: &|_| Err("collection failed".into()),
            infer: |_, _| panic!("inference forbidden"),
            os_type: "fixture",
        };
        assert_eq!(
            get_state(&state, "R1", "all", true).await.unwrap_err(),
            "collection failed"
        );
        let remote = PortableGraph::initialize_at(&root).await.unwrap();
        let result = execute(&remote, "query_state", &request("latest"))
            .await
            .unwrap();
        assert_eq!(result["availability"], "unavailable");
        let id = result["snapshot"]["snapshot_id"].as_str().unwrap();
        assert_eq!(
            execute(&remote, "query_state", &request(id)).await.unwrap(),
            result
        );
    }
    #[test]
    fn output_budget_and_registry_contract() {
        assert!(bounded_output(json!({"value":"x".repeat(1024*1024)})).is_err());
        let tools = crate::portable_device::ReadOnlyToolRegistry::default().tools();
        for name in ["query_state", "diff_state"] {
            assert!(tools.iter().any(|t| t.id == name && t.read_only));
            let kind: mikomai_core::tool_kind::ToolKind = name.parse().unwrap();
            assert!(kind.is_read_only());
            assert!(!kind.is_device_target_tool());
        }
    }

    #[test]
    fn catalog_identities_are_reused_and_inputs_are_strict() {
        assert_eq!(
            resource_shape("ospf").unwrap().identity,
            router_schema::resource_schema("ospf").unwrap().identity
        );
        assert!(resource_shape("arbitrary").is_err());
        let mut args = request("latest");
        args["refresh"] = json!(true);
        assert!(serde_json::from_value::<QueryStateInput>(args).is_err());
    }
}
