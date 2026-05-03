import os
import lancedb
import sys
import json
from sentence_transformers import SentenceTransformer

# Configuration
DB_PATH = os.environ.get("MIKOMAI_DB_PATH", os.path.expanduser("~/Library/Application Support/com.mikomai.agent/lancedb"))
TABLE_NAME = "documents"
MODEL_NAME = "all-MiniLM-L6-v2"

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "No query provided"}))
        return

    query = sys.argv[1]
    
    try:
        db = lancedb.connect(DB_PATH)
        table = db.open_table(TABLE_NAME)
        
        model = SentenceTransformer(MODEL_NAME)
        query_vector = model.encode(query).tolist()
        
        results = table.search(query_vector).limit(3).to_list()
        
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
