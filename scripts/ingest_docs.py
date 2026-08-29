import os
import lancedb
import pandas as pd
import frontmatter
import numpy as np
from sentence_transformers import SentenceTransformer
from pathlib import Path
import re

# Configuration
DOCS_DIR = os.environ.get("MIKOMAI_DOCS_DIR", "./nw-docs")
# Keep the command-line ingestion target aligned with the desktop application's
# default.  An explicit MIKOMAI_DB_PATH remains the single override point.
DB_PATH = os.environ.get(
    "MIKOMAI_DB_PATH",
    os.path.expanduser("~/Library/Application Support/com.mikomai.agent/lancedb"),
)
TABLE_NAME = "documents"
CHUNK_SIZE = int(os.environ.get("MIKOMAI_CHUNK_SIZE", "1400"))
CHUNK_OVERLAP = int(os.environ.get("MIKOMAI_CHUNK_OVERLAP", "180"))

MODEL_NAME = "intfloat/multilingual-e5-large-instruct"
MODEL_CACHE_PATH = os.environ.get(
    "MIKOMAI_MODEL_CACHE_PATH",
    os.path.expanduser("~/Library/Application Support/com.mikomai.agent/model_cache")
)

def split_chunks(content: str, chunk_size: int = CHUNK_SIZE, overlap: int = CHUNK_OVERLAP):
    """Split Markdown on headings first, then into overlapping bounded chunks.

    The heading is retained in every chunk so command snippets do not lose their
    vendor/operation context during retrieval.
    """
    if chunk_size <= 0 or overlap < 0 or overlap >= chunk_size:
        raise ValueError("chunk_size must be positive and overlap must be smaller than chunk_size")

    sections = re.split(r"(?=^#{1,6}\s+)", content, flags=re.MULTILINE)
    chunks = []
    for section in (s.strip() for s in sections):
        if not section:
            continue
        if len(section) <= chunk_size:
            chunks.append(section)
            continue
        start = 0
        while start < len(section):
            end = min(len(section), start + chunk_size)
            if end < len(section):
                boundary = max(section.rfind("\n", start, end), section.rfind("。", start, end))
                if boundary > start + chunk_size // 2:
                    end = boundary + 1
            chunks.append(section[start:end].strip())
            if end == len(section):
                break
            start = end - overlap
    return chunks


def main():
    print(f"Initializing LanceDB at {DB_PATH}...")
    db = lancedb.connect(DB_PATH)
    
    model_dir = os.path.join(MODEL_CACHE_PATH, MODEL_NAME.replace("/", "_"))
    if os.path.exists(model_dir):
        print(f"Loading embedding model from local cache: {model_dir}...")
        model = SentenceTransformer(model_dir)
    else:
        print(f"Downloading and loading embedding model: {MODEL_NAME}...")
        model = SentenceTransformer(MODEL_NAME)
        print(f"Saving embedding model to local cache: {model_dir}...")
        os.makedirs(model_dir, exist_ok=True)
        model.save(model_dir)
    
    data = []
    
    print(f"Scanning directory: {DOCS_DIR}...")
    docs_path = Path(DOCS_DIR)
    
    for md_file in docs_path.rglob("*.md"):
        print(f"Processing: {md_file}")
        with open(md_file, "r", encoding="utf-8") as f:
            post = frontmatter.load(f)
            
            # Metadata
            metadata = post.metadata
            content = post.content

            # Replace placeholders like {brand} with values from metadata
            for key, value in metadata.items():
                placeholder = f"{{{key}}}"
                if placeholder in content:
                    content = content.replace(placeholder, str(value))
            
            for chunk_index, chunk in enumerate(split_chunks(content)):
                # E5 models require "passage: " prefix for documents.
                embedding = model.encode(f"passage: {chunk}").astype(np.float32)
                data.append({
                    "id": f"{md_file}:{chunk_index}",
                    "chunk_index": chunk_index,
                    "vector": embedding,
                    "text": chunk,
                    "path": str(md_file),
                    "brand": metadata.get("brand", ""),
                    "os_version": metadata.get("os_version", ""),
                    "category": metadata.get("category", ""),
                    "command_type": metadata.get("command_type", ""),
                    "target_model": metadata.get("target_model", "")
                })

    if not data:
        print("No documents found.")
        return

    print(f"Creating/Updating table '{TABLE_NAME}' with {len(data)} documents...")
    df = pd.DataFrame(data)
    
    # Overwrite the table if it exists
    table = db.create_table(TABLE_NAME, data=df, mode="overwrite")
    
    # Create optimized index
    # Note: LanceDB requires more vectors than partitions for IVF index
    actual_partitions = 256
    if len(df) < actual_partitions:
        actual_partitions = max(1, len(df) // 4)
        print(f"Dataset too small for 256 partitions. Using {actual_partitions} partitions instead.")

    print(f"Creating IVF-PQ index (cosine, partitions={actual_partitions}, sub_vectors=128)...")
    try:
        table.create_index(
            metric="cosine",
            num_partitions=actual_partitions,
            num_sub_vectors=128
        )
    except Exception as e:
        print(f"Warning: Could not create IVF-PQ index: {e}")

    print("Creating Full-Text Search (FTS) index on 'text' column...")
    try:
        table.create_fts_index("text", replace=True)
        print("FTS index created successfully.")
    except Exception as e:
        print(f"Warning: Could not create FTS index: {e}")
    
    # Optimize table (compact files, cleanup old versions, etc.)
    print("Optimizing table...")
    table.optimize()
    
    print("Ingestion complete.")

if __name__ == "__main__":
    main()
