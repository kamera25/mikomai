use std::fs;
use std::path::PathBuf;
use mikomai_lib::snapshot::SnapshotManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SnapshotManager Demo ===");

    // We will use a dedicated demo directory under the workspace target/ or a local folder
    // to avoid cluttering the system's AppData directory during manual validation.
    // If you want to use the default AppData directory, you would call `SnapshotManager::new()?`
    let demo_base_dir = std::env::current_dir()?.join("demo_storage");
    
    // Clean up any previous runs
    if demo_base_dir.exists() {
        fs::remove_dir_all(&demo_base_dir)?;
    }

    println!("Using storage directory: {}\n", demo_base_dir.display());
    let mut manager = SnapshotManager::with_base_dir(demo_base_dir.clone());

    // --- Scenario 1: Initial pull of Router-A and Router-B ---
    println!("[Scenario 1] Fetching configs for Router-A and Router-B at 17:00 JST simulation...");
    let snap_dir_1 = manager.create_snapshot_dir()?;
    println!("Created Snapshot Directory: {}", snap_dir_1.display());

    let file_a_1 = manager.save_artifact("Router-A", "config", "Router-A Configuration [Initial - 17:00 JST]")?;
    let file_b_1 = manager.save_artifact("Router-B", "config", "Router-B Configuration [Initial - 17:00 JST]")?;
    println!("Saved: {}", file_a_1.display());
    println!("Saved: {}", file_b_1.display());

    println!("Updating storage/current...");
    manager.update_current_link(&snap_dir_1)?;

    println!("Current Folder contents:");
    print_directory_contents(&demo_base_dir.join("current"))?;
    println!();

    // --- Scenario 2: Collision & Suffix test / Router-A only update ---
    println!("[Scenario 2] Fetching config for Router-A only (simulating changes a bit later)...");
    println!("(Since this runs in the same second, collision resolution will kick in!)");
    let snap_dir_2 = manager.create_snapshot_dir()?;
    println!("Created Snapshot Directory (Collision resolved): {}", snap_dir_2.display());

    let file_a_2 = manager.save_artifact("Router-A", "config", "Router-A Configuration [Updated - 17:15 JST Simulation]")?;
    println!("Saved: {}", file_a_2.display());

    println!("Updating storage/current (merging Router-A updates)...");
    manager.update_current_link(&snap_dir_2)?;

    println!("Current Folder contents (Router-A is updated, Router-B remains unchanged):");
    print_directory_contents(&demo_base_dir.join("current"))?;
    println!();

    // --- Scenario 3: Custom file extensions ---
    println!("[Scenario 3] Saving ARP data for Router-A with a JSON extension...");
    let file_arp = manager.save_artifact("Router-A", "arp.json", r#"{ "arp_table": [ { "ip": "192.168.1.1", "mac": "00:11:22:33:44:55" } ] }"#)?;
    println!("Saved: {}", file_arp.display());
    
    println!("Updating storage/current with ARP data...");
    // Update using the active snapshot directory
    if let Some(active_dir) = manager.current_snapshot_dir() {
        manager.update_current_link(active_dir)?;
    }

    println!("Current Folder contents:");
    print_directory_contents(&demo_base_dir.join("current"))?;
    println!();

    println!("=== Demo Completed Successfully ===");
    Ok(())
}

fn print_directory_contents(dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.exists() {
        println!("  [Directory does not exist]");
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let content = fs::read_to_string(&path)?;
            let preview = if content.len() > 80 {
                format!("{}...", &content[..80])
            } else {
                content.replace('\n', " ")
            };
            println!("  - {} -> \"{}\"", entry.file_name().to_string_lossy(), preview);
        }
    }
    Ok(())
}
