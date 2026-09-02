use crate::background_work::BackgroundWorkState;
use crate::graph::GraphDataKind;
use crate::mcp::fetch::fetch_arp::ArpFetcher;
use crate::mcp::fetch::fetch_base::McpCommandFetcher;
use crate::mcp::fetch::fetch_config::ConfigFetcher;
use crate::mcp::fetch::fetch_routing::RoutingFetcher;
use crate::mcp::fetch::get_state::{
    BgpFetcher, InterfacesFetcher, LldpFetcher, MacTableFetcher, OspfFetcher,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRefreshStarted {
    pub node_count: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeRefreshFinished {
    node_count: usize,
    successful_fetches: usize,
    failed_fetches: usize,
}

async fn collect<F: McpCommandFetcher>(
    app: &tauri::AppHandle,
    priority: &BackgroundWorkState,
    device: &str,
    fetcher: F,
    kind: GraphDataKind,
) -> Result<(), String> {
    priority.wait_for_foreground_idle().await;
    let result = fetcher.fetch_device_info(app, device).await?;
    if !result.success || result.output.trim().is_empty() {
        return Err(if result.output.trim().is_empty() {
            "The device returned no data".to_string()
        } else {
            result.output
        });
    }
    app.state::<crate::graph::SurrealDbState>()
        .ingest(crate::graph::GraphIngestInput {
            source_id: format!("settings.node_db_bulk_refresh.{}", kind.as_str()),
            collected_at: chrono::Utc::now(),
            device_name: device.to_string(),
            kind,
            raw: result.output,
            normalized: None,
            canonical: None,
            evidence: None,
            normalizer_version: "bulk-raw-v1".to_string(),
        })
        .await
}

#[tauri::command]
pub async fn start_node_db_bulk_refresh(
    app: tauri::AppHandle,
    priority: tauri::State<'_, BackgroundWorkState>,
) -> Result<NodeRefreshStarted, String> {
    let refresh_guard = priority
        .try_begin_node_refresh()
        .ok_or_else(|| "A node database refresh is already running".to_string())?;
    let devices: Vec<String> = crate::connections::load_connections_raw(&app)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|connection| connection.hostname.as_str().to_string())
        .filter(|hostname| !hostname.trim().is_empty())
        .collect();
    let node_count = devices.len();
    if node_count == 0 {
        return Err("No registered nodes were found".to_string());
    }

    let priority = priority.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _refresh_guard = refresh_guard;
        let mut successful_fetches = 0;
        let mut failed_fetches = 0;
        for device in &devices {
            macro_rules! run_collector {
                ($fetcher:expr, $kind:expr) => {
                    match collect(&app, &priority, device, $fetcher, $kind).await {
                        Ok(()) => successful_fetches += 1,
                        Err(error) => {
                            failed_fetches += 1;
                            log::warn!("Node DB bulk refresh failed for {device}: {error}");
                        }
                    }
                };
            }
            run_collector!(ConfigFetcher, GraphDataKind::Config);
            run_collector!(RoutingFetcher, GraphDataKind::Routing);
            run_collector!(ArpFetcher, GraphDataKind::Arp);
            run_collector!(InterfacesFetcher, GraphDataKind::Interfaces);
            run_collector!(LldpFetcher, GraphDataKind::Lldp);
            run_collector!(MacTableFetcher, GraphDataKind::MacTable);
            run_collector!(BgpFetcher, GraphDataKind::Bgp);
            run_collector!(OspfFetcher, GraphDataKind::Ospf);
        }
        let _ = app.emit(
            "node-db-refresh-finished",
            NodeRefreshFinished {
                node_count,
                successful_fetches,
                failed_fetches,
            },
        );
    });
    Ok(NodeRefreshStarted { node_count })
}
