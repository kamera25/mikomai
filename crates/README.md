# Mikomai workspace crates

依存方向はリポジトリ直下の `mikomai-core/`（domain/application/port）を中心に、`crates/mikomai-adapters`、`crates/mikomai-cli`、`mikomai-desktop/src-tauri` が外向きに依存する形に固定する。coreからTauri、DB、LLM、外部プロセスを参照しない。
