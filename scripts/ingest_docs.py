import os
import lancedb
import pandas as pd
import frontmatter
import numpy as np
from fastembed import TextEmbedding
from pathlib import Path

# Configuration
DOCS_DIR = "./nw-docs"
DB_PATH = os.environ.get("MIKOMAI_DB_PATH", "./data/knowledge.lance")
TABLE_NAME = "documents"

MODEL_NAME = "intfloat/multilingual-e5-large"

def main():
    print(f"Initializing LanceDB at {DB_PATH}...")
    db = lancedb.connect(DB_PATH)
    
    print(f"Initializing fastembed model: {MODEL_NAME}...")
    model = TextEmbedding(model_name=MODEL_NAME)
    
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
            
            # Basic chunking: store the full text and its embedding.
            # E5 models require "passage: " prefix for documents
            instructional_content = f"passage: {content}"
            embeddings = list(model.embed([instructional_content]))
            embedding = np.array(embeddings[0], dtype=np.float32)
            
            data.append({
                "vector": embedding,
                "text": content,
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
    
    # Create optimized index if dataset is large enough
    actual_partitions = 256
    if len(df) < actual_partitions:
        actual_partitions = max(1, len(df) // 4)
        print(f"Dataset size ({len(df)}) is small. Using {actual_partitions} partitions for index.")

    print(f"Creating IVF-PQ index (cosine, partitions={actual_partitions}, sub_vectors=128)...")
    try:
        table.create_index(
            metric="cosine",
            num_partitions=actual_partitions,
            num_sub_vectors=128
        )
    except Exception as e:
        print(f"Warning: Could not create index: {e}")
        print("Continuing without index (small datasets perform well with linear scan).")
    
    # Optimize table (compact files, cleanup old versions, etc.)
    print("Optimizing table...")
    table.optimize()
    
    print("Ingestion complete.")

if __name__ == "__main__":
    main()
