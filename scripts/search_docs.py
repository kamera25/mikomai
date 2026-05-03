import os
import lancedb
import sys
import json
import argparse
from sentence_transformers import SentenceTransformer

# Configuration
DB_PATH = os.environ.get("MIKOMAI_DB_PATH", os.path.expanduser("~/Library/Application Support/com.mikomai.agent/lancedb"))
TABLE_NAME = "documents"
MODEL_NAME = "all-MiniLM-L6-v2"

def main():
    parser = argparse.ArgumentParser(description="Search LanceDB documents")
    parser.add_argument("query", help="The search query string")
    parser.add_argument("--filter", help="Metadata filter string (SQL-like)")
    args = parser.parse_args()

    query = args.query
    filter_str = args.filter
    
    try:
        db = lancedb.connect(DB_PATH)
        table = db.open_table(TABLE_NAME)
        
        model = SentenceTransformer(MODEL_NAME)
        query_vector = model.encode(query).tolist()
        
        search_builder = table.search(query_vector)
        if filter_str:
            search_builder = search_builder.where(filter_str)
            
        results = search_builder.limit(3).to_list()
        
        # Format results for Rust
        formatted_results = []
        for res in results:
            formatted_results.append({
                "text": res["text"],
                "path": res["path"],
                "score": res.get("_distance", 0) # LanceDB uses distance (lower is better)
            })
            
        print(json.dumps(formatted_results))
        
    except Exception as e:
        print(json.dumps({"error": str(e)}))

if __name__ == "__main__":
    main()
