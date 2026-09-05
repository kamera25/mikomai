# RAG evaluation contract

Every answer derived from the knowledge base must expose one or more `根拠`
citation blocks. Each block contains a source path, retrieval rank, and
similarity score. A response without a citation is unsupported and must not be
used as the basis for a device-changing operation.

Before changing embeddings, chunking, metadata filters, or retrieval limits,
evaluate a versioned corpus covering Cisco, Yamaha, FITELnet, device-name and
vendor resolution, troubleshooting retrieval, and no-answer queries. Track
source-at-rank, answer grounding, and unsupported-answer rate.

## Local evaluation procedure

`eval/rag_cases.json` is the versioned retrieval suite. It deliberately has
expected source paths rather than expected prose, so a model change cannot hide
a retrieval regression. Re-ingest the corpus, then run:

```bash
python scripts/rag_eval.py --cases eval/rag_cases.json --report eval/rag-report.json
```

## SurrealDB migration

RAG chunks now live in the embedded SurrealDB store alongside graph and
history data. Rebuild the knowledge base from the versioned Markdown source:

```bash
./ingest.sh
```

Rebuild the managed local knowledge-base store from the versioned Markdown
source whenever the corpus changes.

HNSW keeps its graph in memory. Monitor memory and recall as the corpus grows;
if the knowledge base no longer fits comfortably in memory, migrate the
`rag_chunk_embedding` index to SurrealDB 3.1 DiskANN and rerun this suite.

The command exits non-zero when any case fails and produces Recall@k in the
report. Add production failures as cases before changing the retriever. For
answer-grounding checks, retain the returned citation blocks and verify that
each device-changing proposal has at least one cited chunk above the configured
reranking threshold.

For local-model comparisons, use `scripts/llm_eval.py` with a runner that
applies the same worker system prompt and chat template as the application.
The runner interface is deliberately small: it receives `--model` and
`--prompt`, and writes only the final model output. This allows GGUF variants
and quantization levels to be compared by pass rate without coupling the
repository to a particular local inference executable.
