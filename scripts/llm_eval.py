"""Compare local-model runs using a caller-supplied, non-interactive runner.

The runner receives ``--model PATH --prompt TEXT`` and must print the model's
final answer to stdout. This keeps model files and inference backends outside
the repository while making routing and grounding regressions measurable.
"""
import argparse
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--runner", required=True, help="Executable accepting --model and --prompt")
    parser.add_argument("--model", required=True, help="GGUF path or runner model identifier")
    parser.add_argument("--cases", default="eval/llm_cases.json")
    parser.add_argument("--report", default="eval/llm-report.json")
    args = parser.parse_args()

    cases = json.loads(Path(args.cases).read_text(encoding="utf-8"))
    results = []
    for case in cases:
        completed = subprocess.run(
            [args.runner, "--model", args.model, "--prompt", case["prompt"]],
            check=True, capture_output=True, text=True,
        )
        response = completed.stdout.strip()
        required = case.get("must_contain", [])
        forbidden = case.get("must_not_contain", [])
        passed = all(token in response for token in required) and not any(token in response for token in forbidden)
        results.append({"id": case["id"], "passed": passed, "response": response})

    passed = sum(result["passed"] for result in results)
    report = {"model": args.model, "cases": len(results), "passed": passed,
              "pass_rate": passed / len(results) if results else 0, "results": results}
    Path(args.report).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    raise SystemExit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
