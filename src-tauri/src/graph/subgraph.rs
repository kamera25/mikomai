//! Bounded, read-only traversal of committed graph relationships.
use super::*;
use std::collections::BTreeSet;

#[derive(Debug, Deserialize)]
pub struct SubgraphRequest {
    pub roots: Vec<String>,
    pub depth: u8,
    pub relations: Vec<SubgraphRelation>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubgraphRelation {
    Interface,
    Bgp,
    Vrf,
    Route,
}

impl SubgraphRelation {
    fn kinds(self) -> &'static [&'static str] {
        match self {
            Self::Interface => &["has_interface", "interface"],
            Self::Bgp => &["device_has_bgp", "bgp"],
            Self::Vrf => &["device_has_vrf", "vrf"],
            Self::Route => &["device_has_route", "route"],
        }
    }
}

impl SubgraphRequest {
    fn validate(&self) -> Result<(), String> {
        if self.roots.is_empty()
            || self.roots.len() > 32
            || self.roots.iter().any(|root| root.trim().is_empty())
        {
            return Err("get_subgraph roots must contain 1–32 non-empty device names".into());
        }
        if self.depth > 8 || self.relations.is_empty() {
            return Err("get_subgraph requires depth 0–8 and non-empty relations".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct SubgraphResult {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub missing_roots: Vec<String>,
}

impl SurrealDbState {
    /// Follow selected relationship kinds in both directions using breadth-first
    /// search. Depth zero returns only the existing root devices. Stored node and
    /// edge timestamps/provenance are retained; this does not refresh devices.
    pub async fn get_subgraph(&self, request: SubgraphRequest) -> Result<SubgraphResult, String> {
        request.validate()?;
        let mut result = SubgraphResult {
            nodes: vec![],
            edges: vec![],
            missing_roots: vec![],
        };
        let mut visited = BTreeSet::new();
        for root in request.roots.iter().collect::<BTreeSet<_>>() {
            let nodes = self
                .select("SELECT * FROM device WHERE name = $value;", root)
                .await?;
            if nodes.is_empty() {
                result.missing_roots.push(root.clone());
            } else {
                visited.insert(record_key(root));
            }
        }
        let kinds: Vec<String> = request
            .relations
            .iter()
            .flat_map(|r| r.kinds())
            .map(|kind| (*kind).to_owned())
            .collect();
        let mut frontier = visited.clone();
        let mut edge_keys = BTreeSet::new();
        for _ in 0..request.depth {
            if frontier.is_empty() {
                break;
            }
            let mut response = self.db.query(
                "SELECT * FROM graph_edge WHERE kind IN $kinds AND (from IN $frontier OR to IN $frontier) ORDER BY key LIMIT 2001;"
            ).bind(("kinds", kinds.clone()))
             .bind(("frontier", frontier.into_iter().collect::<Vec<_>>()))
             .await.map_err(|e| format!("Failed to traverse subgraph: {e}"))?;
            let edges: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
            if edges.len() > 2000 {
                return Err("Subgraph exceeds 2000 edges; reduce roots or depth".into());
            }
            frontier = BTreeSet::new();
            for edge in edges {
                for endpoint in ["from", "to"] {
                    if let Some(id) = edge[endpoint].as_str() {
                        if visited.insert(id.to_owned()) {
                            frontier.insert(id.to_owned());
                        }
                    }
                }
                if let Some(key) = edge["key"].as_str() {
                    if edge_keys.insert(key.to_owned()) {
                        result.edges.push(edge);
                    }
                }
            }
            if visited.len() > 1000 || result.edges.len() > 2000 {
                return Err(
                    "Subgraph exceeds 1000 nodes or 2000 edges; reduce roots or depth".into(),
                );
            }
        }
        // Table names are fixed, while record identifiers are bound parameters.
        // Return an explicit traversal identifier because graph_edge endpoints
        // store record keys rather than SurrealDB table-qualified record IDs.
        for table in ["device", "interface", "bgp", "vrf", "route"] {
            let sql = format!("SELECT *, record::id(id) AS traversal_id FROM {table} WHERE record::id(id) IN $ids ORDER BY key;");
            let mut response = self
                .db
                .query(sql)
                .bind(("ids", visited.iter().cloned().collect::<Vec<_>>()))
                .await
                .map_err(|e| format!("Failed to read subgraph nodes: {e}"))?;
            let nodes: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
            for mut node in nodes {
                let node_id = node
                    .as_object_mut()
                    .and_then(|node| node.remove("traversal_id"))
                    .unwrap_or(Value::Null);
                result
                    .nodes
                    .push(json!({"node_id":node_id, "type":table, "record":node}));
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subgraph_rejects_invalid_arguments() {
        for value in [
            json!({"roots":[],"depth":2,"relations":["route"]}),
            json!({"roots":["R1"],"depth":9,"relations":["route"]}),
            json!({"roots":[" "],"depth":2,"relations":["route"]}),
            json!({"roots":["R1"],"depth":2,"relations":[]}),
        ] {
            assert!(serde_json::from_value::<SubgraphRequest>(value)
                .unwrap()
                .validate()
                .is_err());
        }
        assert!(serde_json::from_value::<SubgraphRequest>(
            json!({"roots":["R1"],"depth":-1,"relations":["route"]})
        )
        .is_err());
        assert!(serde_json::from_value::<SubgraphRequest>(
            json!({"roots":["R1"],"depth":2,"relations":["unknown"]})
        )
        .is_err());
    }

    #[tokio::test]
    async fn subgraph_traverses_multiple_roots_with_depth_filters_and_cycles() {
        let path = std::env::temp_dir().join(format!("mikomai-subgraph-{}", uuid::Uuid::new_v4()));
        let state = SurrealDbState::initialize_at(&path).await.unwrap();
        for name in ["R1", "R2", "R3"] {
            state
                .upsert(
                    "device",
                    &record_key(name),
                    json!({"key":record_key(name),"name":name}),
                )
                .await
                .unwrap();
        }
        state
            .upsert(
                "interface",
                &record_key("R1:eth0"),
                json!({"key":"R1:eth0"}),
            )
            .await
            .unwrap();
        for (kind, from, to) in [
            ("has_interface", "R1", "R1:eth0"),
            ("bgp", "R1:eth0", "R3"),
            ("vrf", "R3", "R1"),
            ("device_has_route", "R2", "R3"),
        ] {
            state
                .edge(kind, &record_key(from), &record_key(to), "test")
                .await
                .unwrap();
        }
        for (depth, count) in [(0, 1), (1, 2), (2, 3), (8, 3)] {
            let result = state
                .get_subgraph(SubgraphRequest {
                    roots: vec!["R1".into()],
                    depth,
                    relations: vec![SubgraphRelation::Interface, SubgraphRelation::Bgp],
                })
                .await
                .unwrap();
            assert_eq!(result.nodes.len(), count);
            assert!(result.edges.iter().all(|e| e["kind"] != "vrf"));
        }
        let reverse = state
            .get_subgraph(SubgraphRequest {
                roots: vec!["R3".into()],
                depth: 1,
                relations: vec![SubgraphRelation::Bgp],
            })
            .await
            .unwrap();
        assert_eq!(reverse.nodes.len(), 2);
        assert_eq!(reverse.edges.len(), 1);
        assert!(reverse
            .nodes
            .iter()
            .any(|node| node["node_id"] == record_key("R3")));
        let result = state
            .get_subgraph(SubgraphRequest {
                roots: vec!["R1".into(), "R2".into(), "R1".into(), "missing".into()],
                depth: 8,
                relations: vec![
                    SubgraphRelation::Interface,
                    SubgraphRelation::Bgp,
                    SubgraphRelation::Vrf,
                    SubgraphRelation::Route,
                ],
            })
            .await
            .unwrap();
        assert_eq!(result.nodes.len(), 4);
        assert_eq!(result.edges.len(), 4);
        assert_eq!(result.missing_roots, vec!["missing"]);
        drop(state);
        let _ = std::fs::remove_dir_all(path);
    }
}
