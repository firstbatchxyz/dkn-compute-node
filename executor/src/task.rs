use rig::{
    completion::{CompletionRequest, PromptError},
    message::Message,
};
use serde::{Deserialize, Deserializer};

use crate::{Model, ModelProvider};

/// A future that represents the result of a task execution, of any provider.
pub type TaskResult = Result<String, PromptError>;

/// The body of a task request that includes the messages and the model to use.
///
/// Implements a custom [`Deserialize`] to convert from an object of the form below to self:
///
/// ```ts
/// {
///  "model": string,
///  "messages": { role: string, content: string }[]
/// }
/// ```
///
/// For the `messages` array, the following rules apply:
/// - If the first message is a system message, it will be stored in the `preamble` field.
/// - The last message must be a user message, and it will be stored in the `prompt` field.
/// - All other intermediate messages will be stored in the `chat_history` field.
#[derive(Debug, Clone)]
pub struct TaskBody {
    /// An optional system prompt.
    pub preamble: Option<String>,
    /// The main user prompt.
    pub prompt: Message,
    /// List of messages for context or chat history.
    pub chat_history: Vec<Message>,
    /// The model to use for the task.
    pub model: Model,
}

impl TaskBody {
    /// Creates a new task body with the given prompt and model.
    pub fn new_prompt(prompt: impl Into<String>, model: Model) -> Self {
        TaskBody {
            preamble: None,
            prompt: Message::user(prompt),
            chat_history: Vec::default(),
            model,
        }
    }

    /// Returns whether this task can be executed in parallel, w.r.t to its model.
    pub fn is_batchable(&self) -> bool {
        self.model.provider() != ModelProvider::Ollama
    }
}

impl From<TaskBody> for CompletionRequest {
    fn from(task_body: TaskBody) -> Self {
        CompletionRequest {
            prompt: task_body.prompt,
            preamble: task_body.preamble,
            chat_history: task_body.chat_history,
            documents: Vec::default(),
            tools: Vec::default(),
            temperature: None,
            max_tokens: None,
            additional_params: None,
        }
    }
}

impl<'de> Deserialize<'de> for TaskBody {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct RawMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct RawTaskBody {
            model: String,
            messages: Vec<RawMessage>,
        }

        let raw = RawTaskBody::deserialize(deserializer)?;

        // parse model
        let model = Model::try_from(raw.model).map_err(|err_model| {
            Error::custom(format!("Model {err_model} is not supported by this node."))
        })?;

        // ensure there are messages
        if raw.messages.is_empty() {
            return Err(Error::custom("No messages found in the task body"));
        }

        // ensure the last message is from the user
        if raw.messages.last().unwrap().role != "user" {
            return Err(Error::custom("Last message must be from the user"));
        }

        let mut preamble = None;
        let mut messages = Vec::new();
        for msg in raw.messages.into_iter() {
            match msg.role.as_str() {
                "system" => {
                    // we only expect to see one system message ever
                    if preamble.is_some() {
                        return Err(Error::custom("Only one system message is allowed"));
                    }
                    preamble = Some(msg.content);
                }
                "user" => {
                    messages.push(Message::user(msg.content));
                }
                "assistant" => {
                    messages.push(Message::assistant(msg.content));
                }
                _ => {
                    return Err(Error::custom(format!("Invalid role: {}", msg.role)));
                }
            }
        }

        // the last message (ensured to be role: user), will be returned as the prompt separately
        let prompt = messages.pop().unwrap();

        Ok(TaskBody {
            preamble,
            prompt,
            chat_history: messages,
            model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_task_body_deserialization() {
        let json_data = json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "What is the capital of France?"},
                {"role": "assistant", "content": "The capital of France is Paris."},
                {"role": "user", "content": "How many letters are there in the answer to the last question?"},
            ]
        });

        let task_body: TaskBody = serde_json::from_value(json_data).unwrap();

        assert_eq!(task_body.model, Model::GPT4oMini);
        assert_eq!(
            task_body.preamble,
            Some("You are a helpful assistant.".to_string())
        );
        assert_eq!(task_body.chat_history.len(), 2);
    }
}
