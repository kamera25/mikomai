import os
import lancedb
import pandas as pd
import frontmatter
from sentence_transformers import SentenceTransformer
from pathlib import Path

# Configuration
DOCS_DIR = "./nw-docs"
DB_PATH = "./data/knowledge.lance"
TABLE_NAME = "documents"
MODEL_NAME = "all-MiniLM-L6-v2"

def main():
    print(f"Initializing LanceDB at {DB_PATH}...")
    db = lancedb.connect(DB_PATH)
    
    print(f"Loading embedding model: {MODEL_NAME}...")
    model = SentenceTransformer(MODEL_NAME)
    
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
            
            # Basic chunking: for now, we just take the whole content if it's small, 
            # or we could split by headers. Let's keep it simple for the first version.
            # We'll store the full text and its embedding.
            
            embedding = model.encode(content).tolist()
            
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
    db.create_table(TABLE_NAME, data=df, mode="overwrite")
    
    print("Ingestion complete.")

if __name__ == "__main__":
    main()
