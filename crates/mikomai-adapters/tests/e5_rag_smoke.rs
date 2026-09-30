//! Opt-in real model smoke. Downloads/loads multilingual E5-Large (about 2.3 GB)
//! into the same cache used by `FastEmbedE5`, then exercises actual F220 retrieval.

use mikomai_adapters::{
    e5_embedder::FastEmbedE5,
    portable_graph::PortableGraph,
    portable_rag::{PortableRag, RagEmbedder},
};
use std::{fs, path::PathBuf, sync::Arc};

#[tokio::test]
#[ignore = "downloads/loads the real multilingual E5-Large model (~2.3 GB)"]
async fn multilingual_e5_embeds_and_retrieves_f220_vlan_with_vendor_filter_and_citation() {
    let embedder = Arc::new(FastEmbedE5::new());
    let vector = embedder
        .embed(&["passage: F220 access VLAN 設定".to_owned()])
        .expect("multilingual E5-Large should initialize and embed");
    assert_eq!(vector.len(), 1);
    assert_eq!(vector[0].len(), 1024);
    assert!(vector[0].iter().all(|value| value.is_finite()));
    assert!(vector[0].iter().any(|value| value.abs() > 1e-8));
    println!("real E5 embedding verified: {} dimensions", vector[0].len());

    let db_path =
        std::env::temp_dir().join(format!("mikomai-e5-rag-smoke-{}", uuid::Uuid::new_v4()));
    let graph = PortableGraph::initialize_at(&db_path)
        .await
        .expect("temporary graph database should initialize");
    let rag = PortableRag::new(graph.clone(), embedder);
    let fitelnet_docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nw-docs/fitelnet");
    let ingested = rag
        .ingest_path(&fitelnet_docs)
        .await
        .expect("Fitelnet manuals should be embedded and ingested");
    assert!(
        ingested >= 3,
        "expected all F220 manuals, got {ingested} chunks"
    );

    // A deliberately competing vendor document proves the brand filter is
    // applied inside the same vector/full-text corpus.
    let cisco_doc = db_path.join("cisco-f220-decoy.md");
    fs::write(
        &cisco_doc,
        "---\nbrand: Cisco\ncategory: configuration\n---\n# Cisco F220 VLAN decoy\nF220 access port VLAN configuration uses vlan-id and channel-group commands.",
    )
    .expect("write competing vendor fixture");
    rag.ingest_path(&cisco_doc)
        .await
        .expect("competing vendor fixture should ingest");

    let query = "FITELnet F220でアクセスポートにVLANを設定する手順とコマンドは何ですか";
    let result = rag
        .search(query, Some("Fitelnet"))
        .await
        .expect("E5-backed filtered search should complete");
    assert!(result.success);
    assert!(
        result
            .citations
            .iter()
            .any(|citation| citation.source_path.ends_with("02-2_make_access_vlan.md")),
        "expected the F220 access VLAN manual citation; got: {:?}\n{}",
        result.citations,
        result.output
    );
    assert!(result
        .citations
        .iter()
        .all(|citation| !citation.source_path.ends_with("cisco-f220-decoy.md")));
    assert!(result.output.contains("根拠 [1]"));
    assert!(result.output.contains("vlan-id"));
    println!("F220 query: {query}");
    println!("citations: {:#?}", result.citations);
    println!("retrieval output:\n{}", result.output);

    drop(rag);
    drop(graph);
    fs::remove_dir_all(&db_path).expect("remove temporary graph and fixture");
}
