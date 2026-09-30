# Mikomai regression baseline

This is the current portable Swift/FFI migration baseline. The cases are test contracts, not a proposed redesign. A future change that intentionally alters one of these behaviors requires an explicit baseline update.

| Priority | Case | Expected behavior | Automated check |
| --- | --- | --- | --- |
| P0 | Approval hash and claim gate | Exact plan hash is required and a claimed plan cannot be claimed twice | `approval_boundary::tests::ffi_boundary_requires_matching_approval_and_claims_execution_once` |
| P0 | Durable execution claim | A consumed operation id is persisted and remains consumed after runtime reinitialization | `approved_write_claim_survives_runtime_reinitialization` |
| P1 | MAC normalization | Cisco dotted and short colon MACs normalize to lowercase colon notation; local ARP requires explicit local intent | `dispatch::tests::canonicalizes_legacy_arp_mac_shortcut_targets` |
| P1 | Planner schema restriction | MAC lookup schema limits the allowed tools and pins the canonical MAC value | `planner::tests::narrows_mac_lookup_schema_to_arp_and_optional_reachability_tools` |
| P1 | Choice continuation | User choice preserves the task id and goal, adds human evidence, and continues to the next safety tool | `choice_resume_keeps_agent_task_and_continues_to_the_next_tool` |
| P1 | DHCP preview | Prepare request returns a bounded preview and never sends a packet | `network::packet::tests::dhcp_request_preview_is_valid_and_never_exposes_identity_or_transmits` |
| P2 | Tool failure remains failure | A failed tool result cannot become success evidence or a successful final answer | `application::tests::failed_tool_result_cannot_become_success_evidence_or_final_answer` |
| P3 | F220 VLAN CLI answer | Exit 0; answer uses Fitelnet VLAN documentation, cites Access VLAN source, and filters out Cisco decoys | `npm run cli -- chat "F220のVLAN設定方法を教えて"` plus marker checks |

## Scope limits

The Rust workspace suite includes fake and loopback transport checks. The real E5 smoke separately initializes multilingual E5, indexes the F220 documents, and checks vendor filtering and citations. CLI validates the configured local model/RAG path; it does not prove Swift UI behavior or real-device reachability/configuration. Those remain unverified unless tested on a specifically authorized device.
