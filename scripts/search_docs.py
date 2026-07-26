import os
import lancedb
import sys
import json
import argparse
from sentence_transformers import SentenceTransformer

# Configuration
DB_PATH = os.environ.get("MIKOMAI_DB_PATH", os.path.expanduser("~/Library/Application Support/com.mikomai.agent/lancedb"))
TABLE_NAME = "documents"
MODEL_NAME = "intfloat/multilingual-e5-large-instruct"
MODEL_CACHE_PATH = os.environ.get(
    "MIKOMAI_MODEL_CACHE_PATH",
    os.path.expanduser("~/Library/Application Support/com.mikomai.agent/model_cache")
)

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
        
        import numpy as np
        model_dir = os.path.join(MODEL_CACHE_PATH, MODEL_NAME.replace("/", "_"))
        if os.path.exists(model_dir):
            model = SentenceTransformer(model_dir)
        else:
            model = SentenceTransformer(MODEL_NAME)
            os.makedirs(model_dir, exist_ok=True)
            model.save(model_dir)
        # E5 instruct format for query
        task_description = "ネットワーク機器の操作マニュアルから、関連する設定コマンドや手順を検索します。"
        instructional_query = f"Instruct: {task_description}\nQuery: {query}"
        query_vector = model.encode(instructional_query).astype(np.float16)
        
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
