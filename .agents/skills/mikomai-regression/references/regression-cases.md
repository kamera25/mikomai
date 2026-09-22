# Mikomai regression baseline

This is the baseline captured from the current implementation and the preceding MAC/Graph investigation. The baseline is a test contract, not a proposed redesign. A future change that intentionally alters one of these responses requires an explicit baseline update.

| Priority | Case | Input / fixture | Current response treated as correct | Automated check |
|---|---|---|---|---|
| P0 | Repeated Graph query is cut off | Two consecutive `query_network_graph` decisions with the same arguments | The second identical query is not executed; the task finishes with a clear “new observation is unavailable” result | `stops_consecutive_identical_graph_queries_but_allows_a_changed_query` |
| P1 | One digit MAC octet | `0:2b:f5:3c:cc:7c` + “応答があるか” | Normalize to `00:2b:f5:3c:cc:7c`, call `find_ip_by_mac`, then call `self_network_ping` after an IP is found; do not use free-text `query_network_graph` | `short_mac_octet_uses_endpoint_lookup_and_then_ping` |
| P1 | Cisco MAC notation | `aaaa.cccc.dddd` + “応答があるか” | Normalize to `aa:aa:cc:cc:dd:dd` and start with `find_ip_by_mac` | `cisco_mac_format_uses_the_same_endpoint_lookup` |
| P1 | Standard colon MAC lookup | `ea:f1:92:50:7b:c3` + “IPアドレスは？” | Use structured `find_ip_by_mac` with the normalized MAC, not a text query | `mac_goal_uses_structured_lookup_instead_of_text_query` |
| P1 | MAC reachability after resolution | A `find_ip_by_mac` result containing `10.0.0.10` | Issue one ping for `10.0.0.10`; after the ping observation, finish with the observed response text | `host_reachability_goal_constrains_query_and_pings_resolved_ip` |
| P2 | Empty Graph result recovery | Empty Graph candidates with registered `NakaokuGW` and `F220` | Query registered devices' ARP state in order, re-run structured MAC lookup, and finish from a match instead of asking for a device immediately | `empty_mac_graph_result_fetches_registered_arp_before_asking_human` |
| P2 | Failed ARP is not absence | ARP tool output is `Execution error: arp failed` | Do not report “MAC does not exist”; report that the result cannot be determined from the failed observation | `failed_local_arp_observation_is_not_reported_as_absent` |
| P2 | Localhost ARP lookup | `localhost のARPテーブル` with a matching entry | Use `get_state(device=localhost, resource=arp, mac=...)` and finish with the matching IP; an empty table reports absence | `localhost_arp_mac_lookup_uses_local_state_and_finishes_from_its_entries` |
| P2 | MAC goal schema restriction | `0:2b:f5:3c:cc:7cから応答があるかチェック` | Planner schema permits `find_ip_by_mac`, `get_state`, and `self_network_ping`, and excludes `query_network_graph`; MAC enum contains the normalized value | `short_mac_reachability_schema_excludes_free_text_graph_queries` |
| P3 | Current CLI knowledge answer | `F220のVLAN設定方法を教えて` | Exit code 0; current output contains the Fitelnet Trunk VLAN and Access VLAN templates and the current third evidence block for `show mac address-table` | `npm run cli -- chat "F220のVLAN設定方法を教えて"` plus marker checks |

## Scope limits

The first nine cases are deterministic planner/harness tests. The tenth captures the current independent CLI response. The CLI does not prove GUI-side LLM inference, SurrealDB behavior, live MCP calls, or real-device reachability. Those areas are **UNVERIFIED** unless a separate authorized live test is run.
