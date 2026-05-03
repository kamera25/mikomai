#!/bin/bash

# Exit on error
set -e

echo "Starting document ingestion to LanceDB..."

# Navigate to project root (where the script is located)
cd "$(dirname "$0")"

# Check if venv exists
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
fi

# Activate venv
source venv/bin/activate

# Install dependencies
echo "Installing/Updating dependencies..."
pip install --upgrade pip
pip install lancedb pyarrow sentence-transformers python-frontmatter pandas numpy pylance

# Determine App Data Directory (macOS default for Tauri)
# Identifier: com.mikomai.agent
APP_DATA_DIR="$HOME/Library/Application Support/com.mikomai.agent"
DB_PATH="$APP_DATA_DIR/lancedb"

echo "Using DB path: $DB_PATH"

# Create directory if it doesn't exist
mkdir -p "$DB_PATH"

# Run the ingestion script with the DB path
MIKOMAI_DB_PATH="$DB_PATH" python3 scripts/ingest_docs.py


echo "Done!"
