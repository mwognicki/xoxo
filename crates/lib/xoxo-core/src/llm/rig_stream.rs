//! Rig-backed streaming implementation for the LLM facade.
//!
//! Produces a stream of [`LlmStreamEvent`]s from the `rig-core` streaming API:
//! text chunks become [`LlmStreamEvent::TextDelta`], reasoning chunks become
//! [`LlmStreamEvent::ThinkingDelta`], and tool-call chunks are silently
//! accumulated by rig until stream end. On stream end the assembled
//! [`LlmCompletionResponse`] (identical in shape to the blocking `send()` path)
//! is emitted as a single [`LlmStreamEvent::Final`].

use futures::stream::{BoxStream, StreamExt, once, unfold};

use crate::chat::structs::{ChatTextMessage, ChatTextRole, CostObservability, TokenUsage};
use crate::config::ProviderConfig;
use crate::llm::facade::{
    LlmCompletionError, LlmCompletionRequest, LlmCompletionResponse, LlmFinishReason,
    LlmStreamEvent, LlmToolCall, to_rig_message, to_rig_tool_choice,
};

type OpenrouterStreamingResponse =
    rig::providers::openrouter::streaming::StreamingCompletionResponse;
type RigOpenrouterStream = rig::streaming::StreamingCompletionResponse<OpenrouterStreamingResponse>;

/// Build a boxed stream that drives an openrouter rig completion to completion,
/// emitting text and thinking deltas as they arrive and a single `Final` event
/// at the end.
pub(crate) fn openrouter_stream(
    provider_config: ProviderConfig,
    request: LlmCompletionRequest,
) -> BoxStream<'static, Result<LlmStreamEvent, LlmCompletionError>> {
    // Build phase is async (client construction + calling `.stream()`), so it
    // lives in a single `once` step. On success we flatten into the per-chunk
    // unfold stream; on failure we short-circuit to a single error item.
    once(async move { start_openrouter_stream(provider_config, request).await })
        .map(|result| match result {
            Ok(stream) => stream,
            Err(error) => {
                Box::pin(once(async move { Err(error) })) as BoxStream<'static, _>
            }
        })
        .flatten()
        .boxed()
}

/// Build the client, issue the streaming request, and hand back a chunk-driven
/// stream ready to be polled for deltas and the final assembled response.
async fn start_openrouter_stream(
    provider_config: ProviderConfig,
    request: LlmCompletionRequest,
) -> Result<BoxStream<'static, Result<LlmStreamEvent, LlmCompletionError>>, LlmCompletionError>
{
    use rig::client::completion::CompletionClient;
    use rig::completion::{CompletionRequestBuilder, ToolDefinition as RigToolDefinition};

    if provider_config.provider_id() != Some("openrouter") {
        return Err(LlmCompletionError::UnsupportedProvider(
            provider_config
                .provider_id()
                .unwrap_or("custom")
                .to_string(),
        ));
    }

    let prompt = request
        .messages
        .last()
        .cloned()
        .ok_or(LlmCompletionError::EmptyRequest)?;
    let history = request
        .messages
        .iter()
        .take(request.messages.len().saturating_sub(1))
        .cloned()
        .map(to_rig_message)
        .collect::<Vec<_>>();
    let tools = request
        .tools
        .iter()
        .map(|tool| RigToolDefinition {
            name: tool.name.clone(),
            description: tool.description.clone().unwrap_or_default(),
            parameters: tool.parameters.clone(),
        })
        .collect::<Vec<_>>();
    let tool_choice = to_rig_tool_choice(&request.tool_choice);
    let model_name = request.model.model_name.clone();
    let provider_name = request.model.provider.name.clone();

    let mut client_builder =
        rig::providers::openrouter::Client::builder().api_key(provider_config.api_key.clone());
    if let Some(base_url) = provider_config.effective_base_url() {
        client_builder = client_builder.base_url(base_url);
    }
    let client = client_builder
        .build()
        .map_err(|error| LlmCompletionError::Execution(error.to_string()))?;
    let model = client.completion_model(model_name.clone());
    let mut builder = CompletionRequestBuilder::new(model, to_rig_message(prompt))
        .messages(history)
        .max_tokens(2048);
    if !tools.is_empty() {
        builder = builder.tools(tools);
    }
    if let Some(choice) = tool_choice {
        builder = builder.tool_choice(choice);
    }
    let rig_stream = builder
        .stream()
        .await
        .map_err(|error| LlmCompletionError::Execution(error.to_string()))?;

    Ok(drive_openrouter_stream(rig_stream, model_name, provider_name).boxed())
}

/// Live-stream state carried between polls of the outer event stream while
/// rig chunks are still arriving.
struct StreamingState {
    rig_stream: RigOpenrouterStream,
    model_name: String,
    provider_name: String,
}

/// State machine for the outer event stream.
///
/// `Streaming` holds the rig inner stream (boxed to avoid the clippy
/// `large_enum_variant` lint against the terminal `Done` variant) while chunks
/// are arriving; transitions to `Done` once the inner stream is exhausted and
/// the `Final` event has been emitted.
enum DriverState {
    Streaming(Box<StreamingState>),
    Done,
}

/// Consume the rig streaming response, yielding one `LlmStreamEvent` per poll:
/// deltas while the inner stream is alive, then a single `Final` synthesised
/// from `rig_stream.choice` + `rig_stream.response` once it ends.
fn drive_openrouter_stream(
    rig_stream: RigOpenrouterStream,
    model_name: String,
    provider_name: String,
) -> impl futures::Stream<Item = Result<LlmStreamEvent, LlmCompletionError>> + Send + 'static {
    let initial = DriverState::Streaming(Box::new(StreamingState {
        rig_stream,
        model_name,
        provider_name,
    }));

    unfold(initial, |state| async move {
        let DriverState::Streaming(mut streaming) = state else {
            return None;
        };

        // Advance through silent chunks (tool-call deltas etc.) so every poll
        // of the outer stream returns either a user-visible event or the final
        // assembled response.
        loop {
            match streaming.rig_stream.next().await {
                Some(Ok(chunk)) => {
                    if let Some(event) = chunk_to_event(chunk) {
                        return Some((Ok(event), DriverState::Streaming(streaming)));
                    }
                    // Silent chunk; keep polling.
                }
                Some(Err(error)) => {
                    return Some((
                        Err(LlmCompletionError::Execution(error.to_string())),
                        DriverState::Done,
                    ));
                }
                None => {
                    let response = assemble_final(
                        &streaming.rig_stream,
                        &streaming.model_name,
                        &streaming.provider_name,
                    );
                    return Some((
                        Ok(LlmStreamEvent::Final(Box::new(response))),
                        DriverState::Done,
                    ));
                }
            }
        }
    })
}

/// Map a single rig streamed chunk to an optional facade stream event.
///
/// Returns `None` when the chunk carries no user-visible delta (tool-call
/// deltas, provider message ids, or the inner `Final` marker — rig folds tool
/// calls and usage into its aggregate state, which we read out at the end).
fn chunk_to_event(
    chunk: rig::streaming::StreamedAssistantContent<OpenrouterStreamingResponse>,
) -> Option<LlmStreamEvent> {
    use rig::streaming::StreamedAssistantContent;

    match chunk {
        StreamedAssistantContent::Text(text) => Some(LlmStreamEvent::TextDelta(text.text)),
        StreamedAssistantContent::Reasoning(reasoning) => {
            Some(LlmStreamEvent::ThinkingDelta(reasoning.display_text()))
        }
        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
            Some(LlmStreamEvent::ThinkingDelta(reasoning))
        }
        StreamedAssistantContent::ToolCall { .. }
        | StreamedAssistantContent::ToolCallDelta { .. }
        | StreamedAssistantContent::Final(_) => None,
    }
}

/// Build the aggregated [`LlmCompletionResponse`] from rig's finalized stream
/// state. Matches the shape produced by the non-streaming `send()` path
/// (message, tool calls, finish reason, observability).
fn assemble_final(
    rig_stream: &RigOpenrouterStream,
    model_name: &str,
    provider_name: &str,
) -> LlmCompletionResponse {
    use rig::completion::GetTokenUsage;
    use rig::message::AssistantContent;

    let message = ChatTextMessage {
        role: ChatTextRole::Agent,
        content: rig_stream
            .choice
            .iter()
            .filter_map(|content| match content {
                AssistantContent::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let tool_calls: Vec<LlmToolCall> = rig_stream
        .choice
        .iter()
        .filter_map(|content| match content {
            AssistantContent::ToolCall(tool_call) => Some(LlmToolCall {
                name: tool_call.function.name.clone(),
                arguments: Some(tool_call.function.arguments.clone()),
            }),
            _ => None,
        })
        .collect();
    let finish_reason = if tool_calls.is_empty() {
        LlmFinishReason::Stop
    } else {
        LlmFinishReason::ToolCalls
    };

    let usage = rig_stream
        .response
        .as_ref()
        .and_then(|response| response.token_usage())
        .unwrap_or_default();

    LlmCompletionResponse {
        message,
        tool_calls,
        finish_reason,
        observability: Some(CostObservability {
            model_name: Some(model_name.to_string()),
            provider_name: Some(provider_name.to_string()),
            usage: TokenUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                reasoning_tokens: 0,
                total_tokens: usage.total_tokens,
            },
            cost: Default::default(),
        }),
    }
}
