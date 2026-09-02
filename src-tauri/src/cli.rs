//! Command-line adapter for the Mikomai runtime.
//!
//! This module intentionally contains no device implementation. It translates
//! CLI arguments into the same intent-oriented application API used by Tauri.

use crate::mcp::fetch::state_resource::StateResource;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::str::FromStr;

#[derive(Debug, Parser)]
#[command(name = "mikomai", version, about = "Mikomai network runtime CLI")]
pub struct Cli {
    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List devices registered by the Mikomai desktop app.
    Devices,
    /// List state resources supported by the runtime.
    Resources,
    /// Observe one kind of state on a registered device.
    GetState {
        /// Registered hostname or IP address.
        device: String,
        /// Semantic resource name, such as arp, routes, or interfaces.
        resource: String,
        /// Optional context retained as evidence for resolution/canonicalization.
        #[arg(long)]
        message: Option<String>,
        /// Output only the raw device response (ignored with --json).
        #[arg(long, value_enum, default_value_t = Output::Pretty)]
        output: Output,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum Output {
    Pretty,
    Raw,
}

#[derive(Serialize)]
struct CliResult<T> {
    ok: bool,
    data: T,
}

pub fn run() -> Result<(), String> {
    run_from(Cli::parse())
}

fn run_from(cli: Cli) -> Result<(), String> {
    crate::logger::init().map_err(|error| error.to_string())?;

    match cli.command {
        Command::Devices => {
            let app = crate::build_app().map_err(|error| error.to_string())?;
            let handle = app.handle().clone();
            let devices =
                crate::connections::load_connections(handle).map_err(|error| error.to_string())?;
            if cli.json {
                print_json(&CliResult {
                    ok: true,
                    data: devices,
                })
            } else if devices.is_empty() {
                println!("No devices are registered.");
                Ok(())
            } else {
                for device in devices {
                    println!(
                        "{}\t{}\t{}",
                        device.hostname,
                        device.ip_string(),
                        device.conn_type
                    );
                }
                Ok(())
            }
        }
        Command::Resources => {
            let resources = StateResource::valid_resources();
            if cli.json {
                print_json(&CliResult {
                    ok: true,
                    data: resources,
                })
            } else {
                println!("{}", resources.join("\n"));
                Ok(())
            }
        }
        Command::GetState {
            device,
            resource,
            message,
            output,
        } => {
            let app = crate::build_app().map_err(|error| error.to_string())?;
            let handle = app.handle().clone();
            let resource = StateResource::from_str(&resource)?;
            let result =
                tauri::async_runtime::block_on(crate::mcp::fetch::get_state::dispatch_get_state(
                    &handle, &device, resource, message,
                ))?;
            if cli.json {
                print_json(&CliResult {
                    ok: result.success,
                    data: result,
                })
            } else if output == Output::Raw {
                print!("{}", result.output);
                Ok(())
            } else {
                if !result.success {
                    return Err(result.output);
                }
                println!("device: {device}");
                println!("resource: {resource}");
                println!("---");
                print!("{}", result.output);
                if !result.output.ends_with('\n') {
                    println!();
                }
                Ok(())
            }
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let output = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    println!("{output}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_intent_oriented_get_state() {
        let cli = Cli::try_parse_from([
            "mikomai",
            "--json",
            "get-state",
            "edge-01",
            "mac-table",
            "--message",
            "uplink only",
        ])
        .unwrap();
        assert!(cli.json);
        assert!(matches!(
            cli.command,
            Command::GetState { device, resource, .. }
                if device == "edge-01" && resource == "mac-table"
        ));
    }
}
