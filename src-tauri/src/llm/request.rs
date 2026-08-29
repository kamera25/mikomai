use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;
use std::num::NonZeroU32;
use tauri::Emitter;
use tauri::Manager;

use crate::llm::llm::{
    is_cancelled, prepare_builder_prompt, prepare_prompt_tokens_with_limit, process_token_bytes,
    replace_interface_abbreviations, LlmError,
};
use crate::llm::llm_manager::SharedModel;
use crate::llm::worker::{LlmWorker, Route};

pub enum InferenceRequest
{
    Initial
    {
        window: tauri::Window,
        prompt: String,
        original_query: String,
        respond_to: tokio::sync::oneshot::Sender<Result<(String, Route), LlmError>>,
    },
    Analyze
    {
        window: tauri::Window,
        user_message: String,
        tool_label: String,
        output: String,
        is_rag: bool,
        is_builder: bool,
        history_block: Option<String>,
        subsequent_task: Option<String>,
        respond_to: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
    },
    Background
    {
        prompt: String,
        app: tauri::AppHandle,
        respond_to: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
    },
    Internal
    {
        prompt: String,
        system_prompt: String,
        schema: Option<String>,
        app: tauri::AppHandle,
        respond_to: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
    },
}

pub trait InferenceRequestHandler
{
    fn handle(self, shared_model: &SharedModel);
    fn reject(self, error: LlmError);
}

impl InferenceRequestHandler for InferenceRequest
{
    fn reject(self, error: LlmError)
    {
        match self
        {
            InferenceRequest::Initial { respond_to, .. } =>
            {
                let _ = respond_to.send(Err(error));
            }
            InferenceRequest::Analyze { respond_to, .. } =>
            {
                let _ = respond_to.send(Err(error));
            }
            InferenceRequest::Background { respond_to, .. } =>
            {
                let _ = respond_to.send(Err(error));
            }
            InferenceRequest::Internal { respond_to, .. } =>
            {
                let _ = respond_to.send(Err(error));
            }
        }
    }

    fn handle(self, shared_model: &SharedModel)
    {
        match self
        {
            InferenceRequest::Initial {
                window,
                prompt,
                original_query,
                respond_to,
            } =>
            {
                let res = handle_initial(shared_model, window, prompt, original_query);
                let _ = respond_to.send(res);
            }
            InferenceRequest::Analyze {
                window,
                user_message,
                tool_label,
                output,
                is_rag,
                is_builder,
                history_block,
                subsequent_task,
                respond_to,
            } =>
            {
                let res = handle_analyze(
                    shared_model,
                    window,
                    user_message,
                    tool_label,
                    output,
                    is_rag,
                    is_builder,
                    history_block,
                    subsequent_task,
                );
                let _ = respond_to.send(res);
            }
            InferenceRequest::Background {
                prompt,
                app,
                respond_to,
            } =>
            {
                let res = handle_background(shared_model, app, prompt);
                let _ = respond_to.send(res);
            }
            InferenceRequest::Internal {
                prompt,
                system_prompt,
                schema,
                app,
                respond_to,
            } =>
            {
                let res = handle_internal(shared_model, app, prompt, system_prompt, schema);
                let _ = respond_to.send(res);
            }
        }
    }
}

pub fn get_worker_for_route(
    shared: &SharedModel,
    route: Route,
) -> Option<&std::sync::Mutex<dyn LlmWorker>>
{
    match route
    {
        // Live investigation is handled by AgentLoop, which owns planning,
        // policy validation, tool execution, and observation.  It must never
        // fall back to a single-turn worker that can only emit tool JSON.
        Route::Investigate => None,
        Route::Knowledge => Some(&shared.knowledge),
        Route::Analysis => Some(&shared.analysis),
        Route::Plotter => Some(&shared.plotter),
        Route::Builder => Some(&shared.builder),
        Route::None => None,
    }
}

fn handle_initial(
    shared_model: &SharedModel,
    window: tauri::Window,
    prompt: String,
    original_query: String,
) -> Result<(String, Route), LlmError>
{
    let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();
    let model = shared_model.model.clone();
    let backend = shared_model.backend.clone();

    let decision = crate::llm::router::RoutingPipeline::route(
        shared_model,
        &original_query,
        &settings,
        window.app_handle(),
    )?;

    match decision.action
    {
        crate::llm::router::RouteAction::StaticReply { message } =>
        {
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(message.clone()),
            );
            Ok((message, Route::None))
        }
        crate::llm::router::RouteAction::DirectToolCall {
            tool_name,
            params,
            message,
        } =>
        {
            let tool_call = serde_json::json!({
                "tool_name": tool_name,
                "params": params
            });
            let response_str = format!(
                "{}\n\n```json\n{}\n```",
                message,
                serde_json::to_string_pretty(&tool_call).unwrap()
            );
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(response_str.clone()),
            );
            Ok((response_str, Route::None))
        }
        crate::llm::router::RouteAction::AskClarification =>
        {
            let ask_msg = crate::llm::router::RoutingPipeline::build_clarification_message();
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::AgentSelected(
                    "MIKOMAI (アシスタント)".to_string(),
                ),
            );
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(ask_msg.clone()),
            );
            Ok((ask_msg, Route::None))
        }
        crate::llm::router::RouteAction::WorkerRoute {
            route,
            subsequent_task,
            ..
        } =>
        {
            let active_route = route;
            let final_prompt = if active_route == Route::Builder
            {
                prepare_builder_prompt(&prompt)
            }
            else
            {
                prompt
            };
            let worker_res = if let Some(worker_mutex) =
                get_worker_for_route(shared_model, active_route)
            {
                let mut worker = worker_mutex.lock().unwrap();
                worker.set_device_contexts(decision.device_contexts);
                let agent_name = worker.agent_name();
                let _ = window.emit(
                    "chat-event",
                    crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()),
                );
                worker
                    .ask(
                        &model,
                        &backend,
                        Some(final_prompt),
                        None,
                        None,
                        None,
                        None,
                        subsequent_task.as_deref(),
                        Some(&window),
                        settings.temperature,
                        settings.repetition_penalty,
                    )
                    .map_err(LlmError::Worker)
            }
            else
            {
                Ok("実行が完了しました。".to_string())
            };
            worker_res.map(|s| (s, active_route))
        }
    }
}

fn handle_analyze(
    shared_model: &SharedModel,
    window: tauri::Window,
    user_message: String,
    tool_label: String,
    output: String,
    is_rag: bool,
    is_builder: bool,
    history_block: Option<String>,
    subsequent_task: Option<String>,
) -> Result<String, LlmError>
{
    let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();
    let model = shared_model.model.clone();
    let backend = shared_model.backend.clone();

    if is_rag
    {
        if is_builder
        {
            let mut worker = shared_model.builder.lock().unwrap();
            let agent_name = worker.agent_name();
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()),
            );

            worker
                .ask(
                    &model,
                    &backend,
                    None,
                    Some(user_message),
                    Some(tool_label),
                    Some(output),
                    history_block,
                    None,
                    Some(&window),
                    settings.temperature,
                    settings.repetition_penalty,
                )
                .map_err(LlmError::Worker)
        }
        else
        {
            let mut worker = shared_model.rag.lock().unwrap();
            let agent_name = worker.agent_name();
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()),
            );

            worker
                .ask(
                    &model,
                    &backend,
                    None,
                    Some(user_message),
                    Some(tool_label),
                    Some(output),
                    history_block,
                    None,
                    Some(&window),
                    settings.temperature,
                    settings.repetition_penalty,
                )
                .map_err(LlmError::Worker)
        }
    }
    else
    {
        let is_ask_user_choice = tool_label.contains("ask_user_choice");
        let is_ask_interface_choice = tool_label.contains("ask_interface_choice");
        let is_ask_ipaddress_choice = tool_label.contains("ask_ipaddress_choice");
        let is_any_choice =
            is_ask_user_choice || is_ask_interface_choice || is_ask_ipaddress_choice;

        if is_any_choice
            && (output.trim() == "cancelled"
                || output.lines().any(|l| l.trim() == "cancelled")
                || output.trim().ends_with("cancelled"))
        {
            log::info!(
                "Choice prompt cancelled by user (tool_label: {}), stopping subsequent sequence.",
                tool_label
            );
            let cancel_msg = "応答が停止されました。";
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(cancel_msg.to_string()),
            );
            return Ok(cancel_msg.to_string());
        }

        let is_nwdiag_tool = tool_label.contains("nwdiag") || tool_label.contains("ネットワーク図");
        let is_nwdiag_success =
            is_nwdiag_tool && output.contains("Network diagram generated successfully");
        let is_nwdiag_failed = is_nwdiag_tool
            && (output.contains("validation failed")
                || output.contains("compilation failed")
                || output.contains("Execution failed"));

        if is_nwdiag_success
        {
            let success_msg = "ネットワーク図の生成が完了しました。";
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(success_msg.to_string()),
            );
            return Ok(success_msg.to_string());
        }

        let (active_route, route_subsequent_task, matched_contexts) = if is_any_choice
        {
            (Route::Builder, None, Vec::new())
        }
        else if is_nwdiag_failed
        {
            (
                Route::Plotter,
                Some("前回のnwdiagスキーマに構文エラーが発生しました。エラーメッセージと指示に従って、正しい構文でnwdiagスキーマを修正・再生成してください。".to_string()),
                Vec::new(),
            )
        }
        else
        {
            let decision = crate::llm::router::RoutingPipeline::route(
                shared_model,
                &user_message,
                &settings,
                window.app_handle(),
            )?;

            match decision.action
            {
                crate::llm::router::RouteAction::AskClarification =>
                {
                    let ask_msg =
                        crate::llm::router::RoutingPipeline::build_clarification_message();
                    let _ = window.emit(
                        "chat-event",
                        crate::mcp::protocol::ChatEvent::AgentSelected(
                            "MIKOMAI (アシスタント)".to_string(),
                        ),
                    );
                    let _ = window.emit(
                        "chat-event",
                        crate::mcp::protocol::ChatEvent::LlmChunk(ask_msg.clone()),
                    );
                    return Ok(ask_msg);
                }
                crate::llm::router::RouteAction::WorkerRoute {
                    route,
                    subsequent_route,
                    subsequent_task,
                    ..
                } =>
                {
                    if let Some(sub_route) = subsequent_route
                    {
                        (sub_route, subsequent_task, decision.device_contexts)
                    }
                    else if is_builder || route == Route::Plotter || route == Route::Builder
                    {
                        (route, subsequent_task, decision.device_contexts)
                    }
                    else
                    {
                        return Ok("実行が完了しました。".to_string());
                    }
                }
                _ =>
                {
                    return Ok("実行が完了しました。".to_string());
                }
            }
        };

        let custom_subsequent_task = if is_ask_user_choice
        {
            Some(format!(
                "ユーザーが「{}」を選択しました。この回答要件を含めてCisco Configを設定・生成してください。",
                output
            ))
        }
        else if is_ask_interface_choice
        {
            Some(format!(
                "ユーザーがインターフェースとして「{}」を選択・入力しました。この情報を反映して設定を生成または変更してください。",
                output
            ))
        }
        else if is_ask_ipaddress_choice
        {
            Some(format!(
                "ユーザーがIPアドレス（およびサブネット）として「{}」を指定・確定しました。この情報を反映して設定を生成または変更してください。",
                output
            ))
        }
        else
        {
            None
        };
        let subsequent_task_ref = if let Some(ref s) = subsequent_task
        {
            Some(s.as_str())
        }
        else if is_any_choice
        {
            custom_subsequent_task.as_deref()
        }
        else
        {
            route_subsequent_task.as_deref()
        };

        let user_message = if active_route == Route::Builder
        {
            replace_interface_abbreviations(&user_message)
        }
        else
        {
            user_message
        };
        let worker_res = if let Some(worker_mutex) =
            get_worker_for_route(shared_model, active_route)
        {
            let mut worker = worker_mutex.lock().unwrap();
            worker.set_device_contexts(matched_contexts);
            let agent_name = worker.agent_name();
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()),
            );
            worker
                .ask(
                    &model,
                    &backend,
                    None,
                    Some(user_message),
                    Some(tool_label),
                    Some(output),
                    history_block,
                    subsequent_task_ref,
                    Some(&window),
                    settings.temperature,
                    settings.repetition_penalty,
                )
                .map_err(LlmError::Worker)
        }
        else
        {
            Ok("実行が完了しました。".to_string())
        };
        worker_res
    }
}

fn handle_background(
    shared_model: &SharedModel,
    app: tauri::AppHandle,
    prompt: String,
) -> Result<String, LlmError>
{
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let model = shared_model.model.clone();
    let backend = shared_model.backend.clone();
    let mut worker = shared_model.summarization.lock().unwrap();

    log::info!("LLM Background Prompt: {}", prompt);
    let res = worker
        .ask(
            &model,
            &backend,
            Some(prompt),
            None,
            None,
            None,
            None,
            None,
            None,
            settings.temperature,
            settings.repetition_penalty,
        )
        .map_err(LlmError::Worker);
    match &res
    {
        Ok(out) => log::info!("LLM Background Response: {}", out),
        Err(e) => log::error!("LLM Background Error: {}", e),
    }
    res
}

fn handle_internal(
    shared_model: &SharedModel,
    app: tauri::AppHandle,
    prompt: String,
    system_prompt: String,
    schema: Option<String>,
) -> Result<String, LlmError>
{
    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        system_prompt, prompt
    );

    log::info!(
        "--- INTERNAL LLM PROMPT ---\n{}\n-------------------------",
        formatted_prompt
    );

    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let n_ctx = settings.n_ctx;
    let max_gen = settings.max_gen;

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(n_ctx as u32));
    ctx_params = ctx_params.with_n_batch(n_ctx as u32);
    ctx_params = ctx_params.with_type_k(llama_cpp_2::context::params::KvCacheType::Q4_0);
    ctx_params = ctx_params.with_type_v(llama_cpp_2::context::params::KvCacheType::Q4_0);
    ctx_params = ctx_params.with_flash_attention_policy(1);

    let mut ctx = shared_model
        .model
        .new_context(&shared_model.backend, ctx_params)
        .map_err(|e| LlmError::ContextCreation(format!("{:?}", e)))?;

    let tokens = prepare_prompt_tokens_with_limit(
        &shared_model.model,
        &formatted_prompt,
        n_ctx,
        max_gen,
        settings.prompt_keep_tokens,
    )?;

    let mut batch = LlamaBatch::new(n_ctx, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate()
    {
        let is_last = i == last_index;
        batch
            .add(token, i as i32, &[0], is_last)
            .map_err(|e| LlmError::BatchAdd(format!("{:?}", e)))?;
    }

    ctx.decode(&mut batch)
        .map_err(|e| LlmError::Decode(format!("{:?}", e)))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();

    let mut samplers = vec![
        LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
    ];

    if let Some(ref schema_str) = schema {
        let grammar_str = llama_cpp_2::json_schema_to_grammar(schema_str)
            .map_err(|e| LlmError::Worker(format!("Failed to convert schema to grammar: {:?}", e)))?;
        let grammar_sampler = LlamaSampler::grammar(&shared_model.model, &grammar_str, "root")
            .map_err(|e| LlmError::Worker(format!("Failed to create grammar sampler: {:?}", e)))?;
        samplers.push(grammar_sampler);
    }

    samplers.push(LlamaSampler::greedy());
    let mut sampler = LlamaSampler::chain_simple(samplers);

    let turn_end_tokens = shared_model
        .model
        .str_to_token("<turn|>", AddBos::Never)
        .unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = max_gen;

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len
    {
        if is_cancelled()
        {
            log::info!("LLM internal loop cancelled");
            break;
        }
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == shared_model.model.token_eos() || Some(new_token_id) == turn_end_token
        {
            break;
        }

        let mut token_bytes = shared_model
            .model
            .token_to_piece_bytes(new_token_id, 256, false, None)
            .unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        process_token_bytes(&mut bytes_accumulator, &mut result_string, None);

        batch.clear();
        batch
            .add(new_token_id, n_cur, &[0], true)
            .map_err(|e| LlmError::BatchAdd(format!("{:?}", e)))?;
        n_cur += 1;

        ctx.decode(&mut batch)
            .map_err(|e| LlmError::Decode(format!("{:?}", e)))?;
    }

    if !bytes_accumulator.is_empty()
    {
        result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
    }

    log::info!(
        "--- INTERNAL LLM RESPONSE ---\n{}\n-------------------------",
        result_string
    );
    Ok(result_string)
}
