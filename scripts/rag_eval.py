"""Evaluate retrieval quality against a versioned, local JSON case set.

Run after ingestion, for example:
  python scripts/rag_eval.py --cases eval/rag_cases.json
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path


def search(query, limit, filter_value=None):
    command = [sys.executable, "scripts/search_docs.py", query, "--limit", str(limit)]
    if filter_value:
        command.extend(["--filter", filter_value])
    completed = subprocess.run(command, check=True, capture_output=True, text=True)
    return json.loads(completed.stdout)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cases", default="eval/rag_cases.json")
    parser.add_argument("--limit", type=int, default=5)
    parser.add_argument("--report", default="eval/rag-report.json")
    args = parser.parse_args()

    cases = json.loads(Path(args.cases).read_text(encoding="utf-8"))
    results = []
    for case in cases:
        hits = search(case["query"], args.limit, case.get("filter"))
        paths = [hit["path"] for hit in hits] if isinstance(hits, list) else []
        expected = case.get("expected_paths", [])
        if case.get("expect_no_answer"):
            passed = not paths or min(hit.get("score", 99) for hit in hits) > 0.85
        else:
            passed = any(path in expected for path in paths)
        results.append({"id": case["id"], "passed": passed, "paths": paths, "expected_paths": expected})

    passed = sum(item["passed"] for item in results)
    report = {
        "cases": len(results),
        "passed": passed,
        "recall_at_k": passed / len(results) if results else 0,
        "results": results,
    }
    Path(args.report).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    raise SystemExit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
