#!/bin/bash

# Exit on error
set -e

echo "Starting document ingestion to SurrealDB..."

# Navigate to project root (where the script is located)
cd "$(dirname "$0")"

cargo run --manifest-path src-tauri/Cargo.toml --bin mikomai-cli -- rag-ingest nw-docs


echo "Done!"
