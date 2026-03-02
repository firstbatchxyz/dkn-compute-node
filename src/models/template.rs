use serde::{Deserialize, Serialize};

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Apply a chat template to a list of messages, producing a formatted prompt string.
///
/// Supported templates: "chatml" (qwen, mistral-nemo), "llama3", "gemma".
/// Falls back to chatml for unknown template names.
pub fn apply_chat_template(template_name: &str, messages: &[ChatMessage]) -> String {
    match template_name {
        "llama3" => format_llama3(messages),
        "gemma" => format_gemma(messages),
        _ => format_chatml(messages), // chatml is the default fallback
    }
}

/// ChatML format used by Qwen, Mistral-Nemo, and others.
/// ```text
/// <|im_start|>system
/// You are a helpful assistant.<|im_end|>
/// <|im_start|>user
/// Hello<|im_end|>
/// <|im_start|>assistant
/// ```
fn format_chatml(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for msg in messages {
        out.push_str(&format!(
            "<|im_start|>{}\n{}<|im_end|>\n",
            msg.role, msg.content
        ));
    }
    out.push_str("<|im_start|>assistant\n");
    out
}

/// Llama 3 instruct format.
/// ```text
/// <|begin_of_text|><|start_header_id|>system<|end_header_id|>
///
/// You are a helpful assistant.<|eot_id|><|start_header_id|>user<|end_header_id|>
///
/// Hello<|eot_id|><|start_header_id|>assistant<|end_header_id|>
///
/// ```
fn format_llama3(messages: &[ChatMessage]) -> String {
    let mut out = String::from("<|begin_of_text|>");
    for msg in messages {
        out.push_str(&format!(
            "<|start_header_id|>{}<|end_header_id|>\n\n{}<|eot_id|>",
            msg.role, msg.content
        ));
    }
    out.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
    out
}

/// Gemma instruct format.
/// ```text
/// <start_of_turn>user
/// Hello<end_of_turn>
/// <start_of_turn>model
/// ```
fn format_gemma(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for msg in messages {
        // Gemma uses "model" instead of "assistant"
        let role = if msg.role == "assistant" {
            "model"
        } else {
            &msg.role
        };
        out.push_str(&format!(
            "<start_of_turn>{}\n{}<end_of_turn>\n",
            role, msg.content
        ));
    }
    out.push_str("<start_of_turn>model\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: "system".into(),
                content: "You are a helpful assistant.".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
            },
        ]
    }

    #[test]
    fn test_chatml_format() {
        let result = apply_chat_template("chatml", &sample_messages());
        assert!(result.contains("<|im_start|>system"));
        assert!(result.contains("You are a helpful assistant.<|im_end|>"));
        assert!(result.contains("<|im_start|>user"));
        assert!(result.contains("Hello<|im_end|>"));
        assert!(result.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_llama3_format() {
        let result = apply_chat_template("llama3", &sample_messages());
        assert!(result.starts_with("<|begin_of_text|>"));
        assert!(result.contains("<|start_header_id|>system<|end_header_id|>"));
        assert!(result.contains("<|start_header_id|>user<|end_header_id|>"));
        assert!(result.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));
    }

    #[test]
    fn test_gemma_format() {
        let msgs = vec![
            ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "Hi there!".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "How are you?".into(),
            },
        ];
        let result = apply_chat_template("gemma", &msgs);
        assert!(result.contains("<start_of_turn>user"));
        // "assistant" should be mapped to "model"
        assert!(result.contains("<start_of_turn>model\nHi there!<end_of_turn>"));
        assert!(result.ends_with("<start_of_turn>model\n"));
    }

    #[test]
    fn test_unknown_template_falls_back_to_chatml() {
        let result = apply_chat_template("unknown-template", &sample_messages());
        assert!(result.contains("<|im_start|>"));
    }

    #[test]
    fn test_chat_message_serde() {
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let roundtrip: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.role, "user");
        assert_eq!(roundtrip.content, "hello");
    }
}
