use crate::bus::{BusEnvelope, BusPayload, Command};
use crate::chat::structs::{
    Chat, ChatEvent, ChatEventBody, ChatLogEntry, ChatTextMessage, ChatTextRole, ChatToolCallId,
    MessageContextState, MessageId, ToolCallCompleted, ToolCallEvent, ToolCallFailed,
    ToolCallKind, ToolCallStarted,
};
use crate::llm::{
    LlmCompletionRequest, LlmCompletionResponse, LlmFacade, LlmFinishReason, LlmStreamEvent, LlmToolCall,
};
use crate::tooling::ToolError;

use futures::StreamExt;
use uuid::Uuid;

use super::HandoffKind;
use super::structs::{AgentRunner, SubagentHandoff};

impl AgentRunner {
    pub(crate) async fn run(mut self) -> Chat {
        while let Some(command) = self.inbound.recv().await {
            match command {
                Command::SubmitUserMessage { .. } => {}
                Command::SendUserMessage { message, .. } => {
                    self.push_message_event(message.clone(), None);
                    let _ = self.events.send(BusEnvelope {
                        path: self.path.clone(),
                        payload: BusPayload::Message(message.clone()),
                    });
                    let _ = self.events.send(BusEnvelope {
                        path: self.path.clone(),
                        payload: BusPayload::Turn(crate::bus::TurnEvent::Started),
                    });

                    loop {
                        let completion = self.complete().await;
                        self.push_message_event(
                            completion.message.clone(),
                            completion.observability.clone(),
                        );
                        let _ = self.events.send(BusEnvelope {
                            path: self.path.clone(),
                            payload: BusPayload::Message(completion.message.clone()),
                        });

                        if completion.finish_reason == LlmFinishReason::Stop {
                            let _ = self.events.send(BusEnvelope {
                                path: self.path.clone(),
                                payload: BusPayload::Turn(crate::bus::TurnEvent::Finished {
                                    reason: completion.finish_reason,
                                }),
                            });
                            break;
                        }

                        let next_message = ChatTextMessage {
                            role: ChatTextRole::User,
                            content: self.dispatch_tool_calls(completion.tool_calls).await,
                        };
                        self.push_message_event(next_message, None);
                    }
                }
                Command::Shutdown { .. } => {
                    let _ = self.events.send(BusEnvelope {
                        path: self.path.clone(),
                        payload: BusPayload::AgentShutdown,
                    });
                    break;
                }
            }
        }

        if let Some(handoff_tx) = self.handoff_tx.take() {
            let summary = self
                .history
                .events
                .iter()
                .rev()
                .find_map(|entry| match &entry.event.body {
                    ChatEventBody::Message(message) if message.role == ChatTextRole::Agent => {
                        Some(message.content.clone())
                    }
                    _ => None,
                });

            let _ = handoff_tx.send(SubagentHandoff {
                kind: HandoffKind::Completed,
                chat: self.history.clone(),
                summary,
                observability: None,
            });
        }

        self.history
    }

    async fn complete(&self) -> LlmCompletionResponse {
        let request = LlmCompletionRequest {
            model: self.blueprint.model.clone(),
            messages: self.completion_messages(),
            tools: self
                .tool_set
                .iter()
                .map(|(name, tool)| crate::llm::LlmToolDefinition {
                    name: name.clone(),
                    description: Some(tool.schema().description.clone()),
                    parameters: tool.schema().parameters.clone(),
                })
                .collect(),
            tool_choice: None,
        };

        if let Some(provider_config) = &self.provider_config {
            let facade = LlmFacade::new();
            let mut stream =
                Box::pin(facade.complete_streaming(provider_config, request.clone()));
            let mut final_response: Option<LlmCompletionResponse> = None;
            let mut stream_error: Option<String> = None;

            while let Some(event) = stream.next().await {
                match event {
                    Ok(LlmStreamEvent::TextDelta(delta)) => {
                        let _ = self.events.send(BusEnvelope {
                            path: self.path.clone(),
                            payload: BusPayload::TextDelta { delta },
                        });
                    }
                    Ok(LlmStreamEvent::ThinkingDelta(delta)) => {
                        let _ = self.events.send(BusEnvelope {
                            path: self.path.clone(),
                            payload: BusPayload::ThinkingDelta { delta },
                        });
                    }
                    Ok(LlmStreamEvent::Final(response)) => {
                        final_response = Some(*response);
                    }
                    Err(error) => {
                        stream_error = Some(error.to_string());
                        break;
                    }
                }
            }

            return match (final_response, stream_error) {
                (Some(response), None) => response,
                (_, Some(message)) => LlmCompletionResponse {
                    message: ChatTextMessage {
                        role: ChatTextRole::Agent,
                        content: message,
                    },
                    tool_calls: Vec::new(),
                    finish_reason: LlmFinishReason::Stop,
                    observability: None,
                },
                (None, None) => LlmCompletionResponse {
                    message: ChatTextMessage {
                        role: ChatTextRole::Agent,
                        content: "completion stream ended without emitting a final response"
                            .to_string(),
                    },
                    tool_calls: Vec::new(),
                    finish_reason: LlmFinishReason::Stop,
                    observability: None,
                },
            };
        }

        self.stub_complete(request).await
    }

    fn completion_messages(&self) -> Vec<ChatTextMessage> {
        let mut messages = vec![ChatTextMessage {
            role: ChatTextRole::System,
            content: self.blueprint.base_prompt.clone(),
        }];

        messages.extend(
            self.history
                .events
                .iter()
                .filter_map(|entry| match &entry.event.body {
                    ChatEventBody::Message(history_message)
                        if history_message.role == ChatTextRole::User
                            || history_message.role == ChatTextRole::Agent =>
                    {
                        Some(history_message.clone())
                    }
                    _ => None,
                }),
        );

        messages
    }

    async fn stub_complete(&self, request: LlmCompletionRequest) -> LlmCompletionResponse {
        let last_user_message = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == ChatTextRole::User)
            .map(|message| message.content.clone())
            .unwrap_or_else(|| "ready".to_string());

        LlmCompletionResponse {
            message: ChatTextMessage {
                role: ChatTextRole::Agent,
                content: format!("stub completion: {last_user_message}"),
            },
            tool_calls: Vec::new(),
            finish_reason: LlmFinishReason::Stop,
            observability: None,
        }
    }

    async fn dispatch_tool_calls(&mut self, calls: Vec<LlmToolCall>) -> String {
        let mut rendered = String::new();
        for call in calls {
            let tool_call_id = ChatToolCallId(Uuid::new_v4().to_string());
            let arguments = call.arguments.clone().unwrap_or(serde_json::Value::Null);

            self.push_tool_call_event(
                ToolCallEvent::Started(ToolCallStarted {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: call.name.clone(),
                    arguments: arguments.clone(),
                    tool_call_kind: ToolCallKind::Generic,
                }),
                None,
            );
            let _ = self.events.send(BusEnvelope {
                path: self.path.clone(),
                payload: BusPayload::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: call.name.clone(),
                    arguments: arguments.clone(),
                    tool_call_kind: ToolCallKind::Generic,
                })),
            });

            let tool = self.tool_set.get(&call.name);
            let outcome = match tool {
                Some(tool) => tool
                    .execute_erased_with_observability(&self.tool_context, arguments)
                    .await,
                None => Err(ToolError::ExecutionFailed(format!(
                    "unknown tool: {}",
                    call.name
                ))),
            };

            let (chat_event, bus_event, rendered_line, observability) = match outcome {
                Ok(result) => {
                    let value = result.output;
                    let preview = tool
                        .map(|tool| tool.map_to_preview(&value))
                        .unwrap_or_else(|| value.to_string());
                    let rendered_value = value.to_string();
                    (
                        ToolCallEvent::Completed(ToolCallCompleted {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result_preview: preview.clone(),
                        }),
                        ToolCallEvent::Completed(ToolCallCompleted {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result_preview: preview.clone(),
                        }),
                        format!("{}: {}", call.name, rendered_value),
                        result.observability,
                    )
                }
                Err(error) => {
                    let message = match error {
                        ToolError::InvalidInput(message) => format!("invalid input: {message}"),
                        ToolError::ExecutionFailed(message) => message,
                    };
                    (
                        ToolCallEvent::Failed(ToolCallFailed {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            message: message.clone(),
                        }),
                        ToolCallEvent::Failed(ToolCallFailed {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            message: message.clone(),
                        }),
                        format!("{} failed: {}", call.name, message),
                        None,
                    )
                }
            };

            self.push_tool_call_event(chat_event, observability);
            let _ = self.events.send(BusEnvelope {
                path: self.path.clone(),
                payload: BusPayload::ToolCall(bus_event),
            });

            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(&rendered_line);
        }
        rendered
    }

    fn push_tool_call_event(
        &mut self,
        event: ToolCallEvent,
        observability: Option<crate::chat::structs::CostObservability>,
    ) {
        let next_id = MessageId(Uuid::new_v4().to_string());
        let parent_id = self.history.events.last().map(|entry| entry.event.id.clone());

        self.history.events.push(ChatLogEntry {
            event: ChatEvent {
                id: next_id.clone(),
                parent_id,
                branch_id: self.history.active_branch_id.clone(),
                body: ChatEventBody::ToolCall(event),
                observability,
            },
            context_state: MessageContextState::Active,
        });

        if let Some(branch) = self
            .history
            .branches
            .iter_mut()
            .find(|branch| branch.id == self.history.active_branch_id)
        {
            branch.head_message_id = Some(next_id);
        }

        self.persist_history_snapshot();
    }

    fn push_message_event(
        &mut self,
        message: ChatTextMessage,
        observability: Option<crate::chat::structs::CostObservability>,
    ) {
        let next_id = MessageId(Uuid::new_v4().to_string());
        let parent_id = self.history.events.last().map(|entry| entry.event.id.clone());

        self.history.events.push(ChatLogEntry {
            event: ChatEvent {
                id: next_id.clone(),
                parent_id,
                branch_id: self.history.active_branch_id.clone(),
                body: ChatEventBody::Message(message),
                observability,
            },
            context_state: MessageContextState::Active,
        });

        if let Some(branch) = self
            .history
            .branches
            .iter_mut()
            .find(|branch| branch.id == self.history.active_branch_id)
        {
            branch.head_message_id = Some(next_id);
        }

        self.persist_history_snapshot();
    }

    fn persist_history_snapshot(&self) {
        if let Err(error) = self.storage.save_chat(&self.history) {
            let _ = self.events.send(BusEnvelope {
                path: self.path.clone(),
                payload: BusPayload::Error(crate::bus::ErrorPayload {
                    message: format!("failed to persist chat snapshot: {error}"),
                }),
            });
        }

        if let Err(error) = self.storage.set_last_used_chat_id(*self.path.root_id()) {
            let _ = self.events.send(BusEnvelope {
                path: self.path.clone(),
                payload: BusPayload::Error(crate::bus::ErrorPayload {
                    message: format!("failed to persist last used chat id: {error}"),
                }),
            });
        }
    }
}
