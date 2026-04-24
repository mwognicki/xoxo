use crate::chat::structs::ChatTextMessage;

use super::types::LlmToolChoice;

pub(crate) fn to_rig_message(message: ChatTextMessage) -> rig::completion::Message {
    match message.role {
        crate::chat::structs::ChatTextRole::System => {
            rig::completion::Message::system(message.content)
        }
        crate::chat::structs::ChatTextRole::User => {
            rig::completion::Message::user(message.content)
        }
        crate::chat::structs::ChatTextRole::Agent => {
            rig::completion::Message::assistant(message.content)
        }
    }
}

pub(crate) fn to_rig_tool_choice(
    choice: &Option<LlmToolChoice>,
) -> Option<rig::message::ToolChoice> {
    choice.as_ref().map(|choice| match choice {
        LlmToolChoice::Auto => rig::message::ToolChoice::Auto,
        LlmToolChoice::None => rig::message::ToolChoice::None,
        LlmToolChoice::Required => rig::message::ToolChoice::Required,
        LlmToolChoice::Specific { function_names } => rig::message::ToolChoice::Specific {
            function_names: function_names.clone(),
        },
    })
}
