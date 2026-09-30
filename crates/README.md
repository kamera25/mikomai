# Mikomai workspace crates

依存方向はリポジトリ直下の `mikomai-core/`（domain/application/port）を中心に、`crates/mikomai-adapters`、`crates/mikomai-cli`、`crates/mikomai-ffi` が外向きに依存する形に固定する。coreからOS固有UI、DB、LLM、外部プロセスの起動方法を参照しない。
