use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Block {
    Text {
        text: String,
    },
    Image {
        media_type: String,
        data_base64: String,
    },
    ToolCall {
        id: String,
        name: String,
        args: Value,
    },
    ToolResult {
        call_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub json_schema: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub streaming: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tools: bool,
    pub vision: bool,
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
    pub image_generation: bool,
    pub verified: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProviderEvent {
    ProviderSelected {
        provider_id: String,
    },
    TextDelta {
        text: String,
    },
    ToolCallComplete {
        id: String,
        name: String,
        args: Value,
    },
    Usage {
        input: u64,
        output: u64,
    },
    Done {
        stop_reason: Option<String>,
    },
    Error {
        kind: String,
        status: Option<u16>,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn neutral_message_blocks_round_trip_without_losing_tool_results() {
        let message = Message {
            role: "assistant".into(),
            blocks: vec![
                Block::Text {
                    text: "Vou consultar.".into(),
                },
                Block::ToolCall {
                    id: "call-1".into(),
                    name: "lookup".into(),
                    args: json!({"query":"status"}),
                },
            ],
        };
        let encoded = serde_json::to_string(&message).unwrap();
        assert_eq!(serde_json::from_str::<Message>(&encoded).unwrap(), message);

        let result = Block::ToolResult {
            call_id: "call-1".into(),
            content: "Encontrado".into(),
            is_error: false,
        };
        assert_eq!(
            serde_json::from_value::<Block>(serde_json::to_value(&result).unwrap()).unwrap(),
            result
        );
    }

    #[test]
    fn provider_events_keep_complete_arguments_and_usage_structured() {
        let call = ProviderEvent::ToolCallComplete {
            id: "call-1".into(),
            name: "lookup".into(),
            args: json!({"query":"status"}),
        };
        let usage = ProviderEvent::Usage {
            input: 7,
            output: 3,
        };
        assert_eq!(
            serde_json::to_value(call).unwrap()["type"],
            "toolCallComplete"
        );
        assert_eq!(serde_json::to_value(usage).unwrap()["input"], 7);
    }
}
