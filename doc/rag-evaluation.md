# RAG evaluation contract

Every answer derived from the knowledge base must expose one or more `根拠`
citation blocks. Each block contains a source path, retrieval rank, and
similarity score. A response without a citation is unsupported and must not be
used as the basis for a device-changing operation.

Before changing embeddings, chunking, metadata filters, or retrieval limits,
evaluate a versioned corpus covering Cisco, Yamaha, FITELnet, device-name and
vendor resolution, troubleshooting retrieval, and no-answer queries. Track
source-at-rank, answer grounding, and unsupported-answer rate.
