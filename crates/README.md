# Mikomai workspace crates

依存方向は `mikomai-core`（domain/application/port）を中心に、`mikomai-adapters`、`mikomai-cli`、`src-tauri` が外向きに依存する形に固定する。coreからTauri、DB、LLM、外部プロセスを参照しない。
