---
name: mikomai-regression
description: Run Mikomai's complete regression suite when the user asks for a regression test, all tests, or a one-shot verification after a bug fix.
metadata:
  short-description: Run Mikomai regression tests
---

# Mikomai regression testing

Use this skill when the user asks to run regression tests, all tests, a one-shot test, or to verify that a Mikomai bug fix did not regress existing behavior.

The current behavior recorded in [regression-cases.md](references/regression-cases.md) is the baseline. Treat a mismatch as a regression even if the new behavior looks more reasonable. Update a baseline only after the user explicitly decides that the new behavior is the desired contract.

## Run

From the Mikomai repository root, run:

```bash
bash .agents/skills/mikomai-regression/scripts/run_regression.sh
```

The script runs the Rust workspace suite, the frontend unit suite, all ten prioritized cases, and the current CLI baseline. It continues after an individual failure, prints a per-check result, and exits nonzero when any check fails. Do not stop after the first failure unless the user asks for a short diagnosis.

Before running, check the working tree and record whether failures come from the current change or from pre-existing local modifications. Do not connect to or modify real network devices as part of this skill. Live MCP/GUI behavior is outside this automated suite and must be reported as unverified.

## Interpret results

- **PASS** means the command completed and the case-specific acceptance condition matched the recorded baseline.
- **FAIL** means the command failed, timed out, or its output/transition differed from the baseline.
- **UNVERIFIED** means the case requires a live device, GUI, LLM, or MCP environment that the script does not exercise.

Report the command, exit status, observed evidence, and baseline comparison for every case. Keep the exact failure output needed to reproduce a failure; do not replace it with a generic summary.

Read the reference file before changing or adding cases. New cases should be deterministic, narrow, and tied to a demonstrated bug or a boundary condition immediately around one.
