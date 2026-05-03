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
pip install lancedb pyarrow sentence-transformers python-frontmatter pandas

# Create data directory if it doesn't exist
mkdir -p data

# Run the ingestion script
python3 scripts/ingest_docs.py

echo "Done!"
