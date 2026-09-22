use mikomai_core::{DesiredStatePatch, EntityRef, Mutation, StateEntity, StateGraph};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = EntityRef::interface("gw", "eth1");
    let current = StateGraph {
        entities: vec![StateEntity {
            target: target.clone(),
            properties: [
                ("admin_state".into(), json!("down")),
                ("mtu".into(), json!(1500)),
            ]
            .into_iter()
            .collect(),
        }],
        relationships: vec![],
    };
    let patch = DesiredStatePatch {
        mutations: vec![Mutation::SetProperty {
            target,
            property: "admin_state".into(),
            value: json!("up"),
        }],
    };
    let desired = patch.apply(&current)?;
    println!("Current: {}", current.entities[0].properties["admin_state"]);
    println!("Desired: {}", desired.entities[0].properties["admin_state"]);
    println!(
        "Diff: {}",
        serde_json::to_string_pretty(&patch.diff(&current)?)?
    );
    Ok(())
}
