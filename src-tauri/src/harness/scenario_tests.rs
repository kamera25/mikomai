//! End-to-end scenario tests for AgentLoop using fake ports.
//!
//! Verifies the four core agent lifecycle paths:
//! 1. 調査成功 (Investigation Success)
//! 2. ポリシー拒否 (Policy Rejection)
//! 3. 承認待ち (Approval / Human Input Pending)
//! 4. ツール失敗 (Tool Failure)

#[cfg(test)]
mod tests {
    use crate::harness::agent_loop::AgentLoop;
    use crate::harness::fake::{FakePlanner, FakeToolExecutor, RecordingReporter};
    use crate::harness::state_machine::HarnessState;
    use crate::mcp::protocol::ChatEvent;
    use crate::network::CommandResult;
    use crate::state::event_log::EventLog;
    use crate::state::events::{ActionType, Decision, HarnessEvent};
    use crate::state::network_state::NetworkState;

    fn make_decision(
        action_type: ActionType,
        objective: &str,
        tool: Option<&str>,
        target: Option<&str>,
        parameters: serde_json::Value,
        reason: Vec<&str>,
        final_answer: Option<&str>,
    ) -> Decision {
        Decision {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type,
            objective: objective.to_string(),
            tool: tool.map(ToString::to_string),
            target: target.map(ToString::to_string),
            parameters,
            reason: reason.into_iter().map(ToString::to_string).collect(),
            expected_observation: Vec::new(),
            final_answer: final_answer.map(ToString::to_string),
        }
    }

    /// 1. シナリオテスト: 調査成功 (Investigation Success)
    /// 計画 -> ツール実行(show ip route) -> 観測記録 -> 最終回答生成 で正常完了する。
    #[tokio::test]
    async fn test_scenario_investigation_success() {
        let goal = "RT1のルーティングテーブルを確認して".to_string();

        // Step 1: Planner decides to execute `network_show` on RT1
        let step1 = make_decision(
            ActionType::Observe,
            "RT1のルーティングテーブルを取得する",
            Some("network_show"),
            Some("RT1"),
            serde_json::json!({ "command": "show ip route" }),
            vec!["RT1の経路情報を取得して現状を把握するため"],
            None,
        );

        // Step 2: Planner receives observation and finishes with diagnostic answer
        let step2 = make_decision(
            ActionType::Finish,
            "RT1のルーティングテーブル確認完了",
            None,
            None,
            serde_json::Value::Null,
            vec![],
            Some("RT1のルーティングテーブルを確認しました。10.0.0.0/24（via 192.168.1.1）への経路が正常に存在します。"),
        );

        let planner = FakePlanner::new(vec![step1, step2]);

        let executor = FakeToolExecutor::new();
        executor.set_tool_result(
            "network_show",
            Ok(CommandResult {
                success: true,
                output: "Codes: C - connected, S - static\nS 10.0.0.0/24 [1/0] via 192.168.1.1\nC 192.168.1.0/24 is directly connected".to_string(),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }),
        );

        let reporter = RecordingReporter::new();
        let mut agent = AgentLoop::new_headless(10);

        let result = agent
            .run_with(goal.clone(), &planner, &executor, &reporter)
            .await;

        // 1. Result verification
        assert!(result.is_ok(), "AgentLoop should finish successfully");
        let final_text = result.unwrap();
        assert!(
            final_text.contains("10.0.0.0/24"),
            "Final text should contain the routed subnet: {}",
            final_text
        );

        // 2. State machine verification
        assert_eq!(agent.state_machine.state(), HarnessState::Finished);
        assert_eq!(agent.state_machine.step_count(), 2);

        // 3. Executor verification
        let executed = executor.executed_tools();
        assert_eq!(executed.len(), 1, "Exactly one tool call should be executed");
        assert_eq!(executed[0].tool, "network_show");
        assert_eq!(
            executed[0].arguments.get("command").and_then(|c| c.as_str()),
            Some("show ip route")
        );

        // 4. NetworkState observation verification
        assert_eq!(
            agent.network_state.observed.observations.len(),
            1,
            "One observation must be recorded in NetworkState"
        );
        let obs = &agent.network_state.observed.observations[0];
        assert!(obs.raw.contains("10.0.0.0/24"));
        assert_eq!(obs.source.device.as_deref(), Some("RT1"));

        // 5. EventLog causal progression verification
        let events = agent.network_state.event_log.events();
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Decision(_))));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Action(_))));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Result(r) if r.observation.raw.contains("10.0.0.0/24"))));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Finished { .. })));

        // 6. Reporter UI events verification
        let chat_events = reporter.chat_events();
        assert!(chat_events.iter().any(|e| matches!(e, ChatEvent::McpInitialStarted(_))));
        assert!(chat_events.iter().any(|e| matches!(e, ChatEvent::AgentSelected(_))));
        assert!(chat_events.iter().any(|e| matches!(e, ChatEvent::McpInitialFinished(_))));
        let commit_logs = reporter.commit_logs();
        assert!(commit_logs.iter().any(|l| l.contains("MCP Tool 実行完了") && l.contains("成功")));
    }

    /// 2. シナリオテスト: ポリシー拒否 (Policy Rejection)
    /// 未承認の設定変更コマンド（network_config 等）を提案した場合、
    /// PolicyValidator によってブロックされ、ToolExecutor は呼び出されずに安全に処理される。
    #[tokio::test]
    async fn test_scenario_policy_rejection() {
        let goal = "ルータの再起動を実施する".to_string();

        // Step 1: Disallowed action attempting configuration command with high-risk reload
        let step1 = make_decision(
            ActionType::Observe,
            "ルータを再起動する",
            Some("network_config"),
            Some("RT1"),
            serde_json::json!({ "command": "reload\ny" }),
            vec!["ルータの再起動を試みる"],
            None,
        );

        // Step 2: Planner finishes acknowledging policy restriction
        let step2 = make_decision(
            ActionType::Finish,
            "ポリシー違反のため停止",
            None,
            None,
            serde_json::Value::Null,
            vec![],
            Some("未承認の設定変更および危険な操作はポリシーにより拒否されました。操作計画の承認が必要です。"),
        );

        let planner = FakePlanner::new(vec![step1, step2]);
        let executor = FakeToolExecutor::new();
        let reporter = RecordingReporter::new();
        let mut agent = AgentLoop::new_headless(10);

        let result = agent
            .run_with(goal, &planner, &executor, &reporter)
            .await;

        assert!(result.is_ok());
        let final_text = result.unwrap();
        assert!(final_text.contains("ポリシーにより拒否"));

        // CRITICAL: Ensure tool was NEVER executed
        let executed = executor.executed_tools();
        assert!(
            executed.is_empty(),
            "Disallowed action must NEVER reach ToolExecutor: {:?}",
            executed
        );

        // Verify reporter emitted agent-warning
        let chunks = reporter.llm_chunks();
        assert!(
            chunks.iter().any(|c| c.contains("agent-warning") && c.contains("ポリシー違反")),
            "Reporter must receive policy warning chunk: {:?}",
            chunks
        );
        assert_eq!(agent.state_machine.state(), HarnessState::Finished);
    }

    /// 3. シナリオテスト: 承認待ち / 人手確認要求 (Approval / Human Input Pending)
    /// (a) 直接 AskHuman を返却する場合
    #[tokio::test]
    async fn test_scenario_approval_pending_direct() {
        let goal = "新しいVLANを作成する".to_string();

        let step1 = make_decision(
            ActionType::AskHuman,
            "追加するVLAN ID (1-4094) を入力してください。",
            None,
            None,
            serde_json::Value::Null,
            vec!["VLAN IDが未指定のためユーザーの指示を仰ぐ"],
            None,
        );

        let planner = FakePlanner::new(vec![step1]);
        let executor = FakeToolExecutor::new();
        let reporter = RecordingReporter::new();
        let mut agent = AgentLoop::new_headless(10);

        let result = agent
            .run_with(goal, &planner, &executor, &reporter)
            .await;

        assert!(result.is_ok());
        let final_text = result.unwrap();
        assert!(
            final_text.starts_with("### ❓ 確認要求"),
            "Result should format human inquiry header: {}",
            final_text
        );
        assert!(final_text.contains("追加するVLAN ID (1-4094) を入力してください。"));

        assert_eq!(agent.state_machine.state(), HarnessState::AskingHuman);
        assert!(executor.executed_tools().is_empty());
    }

    /// 3. シナリオテスト: 承認待ち / 人手確認要求 (Approval / Human Input Pending)
    /// (b) Builder Co-Worker からパラメータ不足で人手確認にハンドオフする場合
    #[tokio::test]
    async fn test_scenario_approval_pending_builder_handoff() {
        // Goal names configuration request to trigger builder delegation
        let goal = "F220 に VLAN を設定する".to_string();

        let step1 = make_decision(
            ActionType::Observe,
            "VLAN設定手順をNW-DBから検索する",
            Some("query_nw_db"),
            None,
            serde_json::json!({ "query": "vlan 設定" }),
            vec!["マニュアルからコマンド構文を取得"],
            None,
        );

        let planner = FakePlanner::new(vec![step1]);
        let executor = FakeToolExecutor::new();
        // Builder returns missing parameter marker
        executor.set_builder_result(
            "query_nw_db",
            Ok("設定に必要なパラメータが不足しています: vlan_id (例: 10) を入力してください。".to_string()),
        );

        let reporter = RecordingReporter::new();
        let mut agent = AgentLoop::new_headless(10);

        let result = agent
            .run_with(goal, &planner, &executor, &reporter)
            .await;

        assert!(result.is_ok());
        let final_text = result.unwrap();
        assert!(
            final_text.starts_with("### ❓ 確認要求"),
            "Result should request user confirmation: {}",
            final_text
        );
        assert!(final_text.contains("不足している値を確認してください"));
        assert!(final_text.contains("vlan_id"));

        assert_eq!(agent.state_machine.state(), HarnessState::AskingHuman);
        // Builder was called once, live device commands were not executed
        assert_eq!(executor.executed_builders().len(), 1);
        assert!(executor.executed_tools().is_empty());
    }

    /// 4. シナリオテスト: ツール失敗 (Tool Failure)
    /// MCPツール実行がエラー（タイムアウトや接続失敗）となった際、
    /// AgentLoop が失敗 Observation を安全に記録し、Planner がそれを受けて診断回答を出力する。
    #[tokio::test]
    async fn test_scenario_tool_failure() {
        let goal = "SW1のポート状態を確認する".to_string();

        let step1 = make_decision(
            ActionType::Observe,
            "SW1のインターフェース情報を取得する",
            Some("network_show"),
            Some("SW1"),
            serde_json::json!({ "command": "show interfaces status" }),
            vec!["ポート状態を確認するため"],
            None,
        );

        let step2 = make_decision(
            ActionType::Finish,
            "SW1接続失敗の診断結果を報告",
            None,
            None,
            serde_json::Value::Null,
            vec![],
            Some("SW1への接続がタイムアウトしました（10000ms経過）。管理IPおよび物理リンクを確認してください。"),
        );

        let planner = FakePlanner::new(vec![step1, step2]);

        let executor = FakeToolExecutor::new();
        // Tool returns command failure
        executor.set_tool_result(
            "network_show",
            Ok(CommandResult {
                success: false,
                output: "Connection timed out: SSH port 22 unreachable on 192.168.1.50".to_string(),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }),
        );

        let reporter = RecordingReporter::new();
        let mut agent = AgentLoop::new_headless(10);

        let result = agent
            .run_with(goal, &planner, &executor, &reporter)
            .await;

        assert!(result.is_ok());
        let final_text = result.unwrap();
        assert!(final_text.contains("SW1への接続がタイムアウトしました"));

        // Observation must record the failure cleanly
        assert_eq!(agent.network_state.observed.observations.len(), 1);
        let obs = &agent.network_state.observed.observations[0];
        assert!(obs.raw.contains("Connection timed out"));

        // Commit logs should indicate failure
        let commit_logs = reporter.commit_logs();
        assert!(
            commit_logs.iter().any(|l| l.contains("成否: 失敗")),
            "Commit log must record tool failure: {:?}",
            commit_logs
        );

        assert_eq!(agent.state_machine.state(), HarnessState::Finished);
        assert_eq!(agent.state_machine.step_count(), 2);
    }

    /// 5. シナリオテスト: 途中終了と再開 (Interruption and Resumption)
    /// (a) ステップ1（情報取得）実行後に中断されたタスクが、途中までのイベントログを保持しており、
    /// 再開時に過去の観察結果を引き継いで完了することを確認する。
    #[tokio::test]
    async fn test_scenario_interruption_and_resumption_state_recovery() {
        let temp_dir = std::env::temp_dir().join(format!("mikomai-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let goal = "RT1のルーティングテーブルを確認して".to_string();

        let step1_observe = make_decision(
            ActionType::Observe,
            "RT1のルーティングテーブルを取得する",
            Some("network_show"),
            Some("RT1"),
            serde_json::json!({ "command": "show ip route" }),
            vec!["ルーティング情報の取得"],
            None,
        );

        let planner1 = FakePlanner::new(vec![step1_observe.clone()]);
        let executor1 = FakeToolExecutor::new();
        executor1.set_tool_result(
            "network_show",
            Ok(CommandResult {
                success: true,
                output: "S 10.0.0.0/24 [1/0] via 192.168.1.1".to_string(),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }),
        );

        let reporter1 = RecordingReporter::new();
        let mut agent1 = AgentLoop::new_headless_with_log_dir(10, temp_dir.clone());

        // Run agent 1 - it will execute step 1 and then stop because planner runs out of decisions
        let result1 = agent1.run_with(goal.clone(), &planner1, &executor1, &reporter1).await;
        assert!(result1.is_ok());
        assert!(result1.unwrap().contains("停止しました"));

        // Verify that intermediate event log was persisted to disk despite interruption!
        let task_id = agent1.network_state.task_id.expect("task_id must be set");
        let log_file = temp_dir.join(format!("{task_id}.json"));
        assert!(log_file.exists(), "Event log must be saved even if task was interrupted");

        let loaded_log = EventLog::load_from_path(&log_file).expect("Must be able to load event log");
        assert!(!loaded_log.is_empty(), "Saved event log must not be empty");

        // Verify that ActionResult in loaded log has idempotency_key
        let has_idempotency_key = loaded_log.events().iter().any(|ev| {
            if let HarnessEvent::Result(res) = ev {
                res.idempotency_key.is_some()
            } else {
                false
            }
        });
        assert!(has_idempotency_key, "ActionResult must contain idempotency_key");

        // Now resume: create a new AgentLoop from the loaded event log
        let restored_state = NetworkState::rebuild_from_log(&loaded_log);
        assert_eq!(restored_state.observed.observations.len(), 1);
        assert!(restored_state.observed.observations[0].raw.contains("10.0.0.0/24"));

        let mut agent2 = AgentLoop::from_saved_state(restored_state, 10);
        let executor2 = FakeToolExecutor::new();

        // Planner on resumption sees the observation and finishes directly
        let step2_finish = make_decision(
            ActionType::Finish,
            "確認完了",
            None,
            None,
            serde_json::Value::Null,
            vec![],
            Some("RT1のルーティングテーブル（10.0.0.0/24）を確認しました"),
        );

        let planner2 = FakePlanner::new(vec![step2_finish]);
        let reporter2 = RecordingReporter::new();

        let result2 = agent2.run_with(goal, &planner2, &executor2, &reporter2).await;
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), "RT1のルーティングテーブル（10.0.0.0/24）を確認しました");

        // Verify that executor2 did NOT need to re-execute any tools
        let executed_tools = executor2.executed_tools();
        assert_eq!(
            executed_tools.len(),
            0,
            "Tools must NOT be unnecessarily re-executed upon resumption!"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// 5. シナリオテスト: 途中終了と再開 (Interruption and Resumption)
    /// (b) すでに実行された変更操作（Configure/Rollback）が再開時に再実行されないことを確認する。
    #[tokio::test]
    async fn test_scenario_interruption_prevents_duplicate_mutation() {
        let temp_dir = std::env::temp_dir().join(format!("mikomai-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let goal = "RT1のインターフェースを設定して".to_string();

        let mut initial_log = EventLog::new();
        let task_id = uuid::Uuid::new_v4();
        initial_log.push(HarnessEvent::TaskStarted {
            task_id,
            timestamp: chrono::Utc::now(),
        });
        initial_log.push(HarnessEvent::GoalSet {
            goal: goal.clone(),
            timestamp: chrono::Utc::now(),
        });

        // Simulate an earlier executed mutating action recorded in event log
        let mutating_action = crate::state::events::Action {
            id: uuid::Uuid::new_v4(),
            decision_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type: ActionType::Configure,
            tool: Some("network_config".to_string()),
            target: Some("RT1".to_string()),
            parameters: serde_json::json!({ "command": "interface Gi0/1\nip address 10.0.0.1 255.255.255.0" }),
        };
        let idempotency_key = mutating_action.compute_idempotency_key();

        let observation = crate::state::events::Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: "Interface configured".to_string(),
            parsed: None,
            source: crate::state::events::ObservationSource {
                device: Some("RT1".to_string()),
                command: None,
                tool_name: Some("network_config".to_string()),
                tool_kind: None,
                parameters: Some(mutating_action.parameters.clone()),
            },
            provenance: crate::state::events::Provenance {
                origin: crate::state::events::ProvenanceOrigin::Tool,
                confidence: Some(1.0),
            },
        };

        initial_log.push(HarnessEvent::Action(mutating_action.clone()));
        initial_log.push(HarnessEvent::Result(crate::state::events::ActionResult {
            id: uuid::Uuid::new_v4(),
            action_id: mutating_action.id,
            timestamp: chrono::Utc::now(),
            success: true,
            observation,
            failure_kind: None,
            idempotency_key: Some(idempotency_key),
            attempt_count: Some(1),
        }));

        // Rebuild state from log
        let restored_state = NetworkState::rebuild_from_log(&initial_log);
        assert!(restored_state.is_mutating_action_already_executed(&mutating_action));

        // Create AgentLoop from restored state
        let mut agent = AgentLoop::from_saved_state(restored_state, 10);
        let executor = FakeToolExecutor::new();

        // Planner proposes the same configure action again, followed by finish
        let step1_dup = make_decision(
            ActionType::Configure,
            "RT1のGigabitEthernet0/1を設定する",
            Some("network_config"),
            Some("RT1"),
            serde_json::json!({ "command": "interface Gi0/1\nip address 10.0.0.1 255.255.255.0" }),
            vec!["インターフェース設定の再試行"],
            None,
        );
        let step2_finish = make_decision(
            ActionType::Finish,
            "完了",
            None,
            None,
            serde_json::Value::Null,
            vec![],
            Some("設定が適用済みであることを確認し完了しました"),
        );

        let planner = FakePlanner::new(vec![step1_dup, step2_finish]);
        let reporter = RecordingReporter::new();

        let result = agent.run_with(goal, &planner, &executor, &reporter).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "設定が適用済みであることを確認し完了しました");

        // CRITICAL: The mutating action was skipped and NEVER re-executed!
        let executed = executor.executed_tools();
        assert!(
            executed.is_empty(),
            "Mutating action must NOT be re-executed on resumption: {:?}",
            executed
        );

        // Verify reporter emitted skip log
        let commits = reporter.commit_logs();
        assert!(
            commits.iter().any(|c| c.contains("変更操作は実行済みのため再実行をスキップしました")),
            "Reporter must log skip message: {:?}",
            commits
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
