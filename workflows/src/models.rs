use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Sequence)]
pub enum Model {
    // Ollama models
    /// [Nous's Hermes-2-Theta model](https://ollama.com/finalend/hermes-3-llama-3.1:8b-q8_0), q8_0 quantized
    #[serde(rename = "finalend/hermes-3-llama-3.1:8b-q8_0")]
    NousTheta,
    /// [Microsoft's Phi3 Medium model](https://ollama.com/library/phi3:medium), q4_1 quantized
    #[serde(rename = "phi3:14b-medium-4k-instruct-q4_1")]
    Phi3Medium,
    /// [Microsoft's Phi3 Medium model, 128k context length](https://ollama.com/library/phi3:medium-128k), q4_1 quantized
    #[serde(rename = "phi3:14b-medium-128k-instruct-q4_1")]
    Phi3Medium128k,
    /// [Microsoft's Phi3.5 Mini model](https://ollama.com/library/phi3.5), 3.8b parameters
    #[serde(rename = "phi3.5:3.8b")]
    Phi3_5Mini,
    /// [Microsoft's Phi3.5 Mini model](https://ollama.com/library/phi3.5:3.8b-mini-instruct-fp16), 3.8b parameters
    #[serde(rename = "phi3.5:3.8b-mini-instruct-fp16")]
    Phi3_5MiniFp16,
    /// [Google's Gemma2 model](https://ollama.com/library/gemma2), 9B parameters
    #[serde(rename = "gemma2:9b-instruct-q8_0")]
    Gemma2_9B,
    /// [Google's Gemma2 model](https://ollama.com/library/gemma2:9b-instruct-fp16), 9B parameters, fp16
    #[serde(rename = "gemma2:9b-instruct-fp16")]
    Gemma2_9BFp16,
    /// [Meta's Llama3.1 model](https://ollama.com/library/llama3.1:latest), 8B parameters
    #[serde(rename = "llama3.1:latest")]
    Llama3_1_8B,
    /// [Meta's Llama3.1 model q8](https://ollama.com/library/llama3.1:8b-text-q8_0)
    #[serde(rename = "llama3.1:8b-instruct-q8_0")]
    Llama3_1_8Bq8,
    /// [Meta's Llama3.1 model fp16](https://ollama.com/library/llama3.1:8b-instruct-fp16)
    #[serde(rename = "llama3.1:8b-instruct-fp16")]
    Llama3_1_8Bf16,
    /// [Meta's Llama3.1 model q4](https://ollama.com/library/llama3.1:8b-text-q4_0)
    #[serde(rename = "llama3.1:8b-text-q4_K_M")]
    Llama3_1_8BTextQ4KM,
    /// [Meta's Llama3.1 model q8](https://ollama.com/library/llama3.1:8b-text-q8_0)
    #[serde(rename = "llama3.1:8b-text-q8_0")]
    Llama3_1_8BTextQ8,
    /// [Meta's Llama3.1 model](https://ollama.com/library/llama3.1:70b), 70B parameters
    #[serde(rename = "llama3.1:70b-instruct-q4_0")]
    Llama3_1_70B,
    /// [Meta's Llama3.1 model q8](https://ollama.com/library/llama3.1:70b-instruct-q8_0)
    #[serde(rename = "llama3.1:70b-instruct-q8_0")]
    Llama3_1_70Bq8,
    /// [Meta's LLama3.1 model fp16](https://ollama.com/library/llama3.1:70b-instruct-fp16)
    #[serde(rename = "llama3.1:70b-text-q4_0")]
    Llama3_1_70BTextQ4KM,
    /// [Meta's LLama3.2 Edge models](https://ollama.com/library/llama3.2/tags), 1B parameters
    #[serde(rename = "llama3.2:1b")]
    Llama3_2_1B,
    /// [Meta's LLama3.2 Edge models](https://ollama.com/library/llama3.2/tags), 3B parameters
    #[serde(rename = "llama3.2:3b")]
    Llama3_2_3B,
    /// [Meta's LLama3.3 Edge models](https://ollama.com/library/llama3.3/tags), 3B parameters
    #[serde(rename = "llama3.3:70b")]
    Llama3_3_70B,
    /// [Meta's LLama3.2 Edge models](https://ollama.com/library/llama3.2/tags), 1B parameters, q4
    #[serde(rename = "llama3.2:1b-text-q4_K_M")]
    Llama3_2_1BTextQ4KM,
    /// [Alibaba's Qwen2.5 model](https://ollama.com/library/qwen2), 7B parameters
    #[serde(rename = "qwen2.5:7b-instruct-q5_0")]
    Qwen2_5_7B,
    /// [Alibaba's Qwen2.5 model](https://ollama.com/library/qwen2), 7B parameters, fp16
    #[serde(rename = "qwen2.5:7b-instruct-fp16")]
    Qwen2_5_7Bf16,
    /// [Alibaba's Qwen2.5 model](https://ollama.com/library/qwen2), 32B parameters, fp16
    #[serde(rename = "qwen2.5:32b-instruct-fp16")]
    Qwen2_5_32Bf16,
    /// [Alibaba's Qwen2.5 Coder]
    #[serde(rename = "qwen2.5-coder:1.5b")]
    Qwen2_5Coder1_5B,
    /// [AliBaba's Qwen2.5 7b]
    #[serde(rename = "qwen2.5-coder:7b-instruct")]
    Qwen2_5coder7B,
    /// [AliBaba's Qwen2.5 7b 8bit]
    #[serde(rename = "qwen2.5-coder:7b-instruct-q8_0")]
    Qwen2_5oder7Bq8,
    /// [AliBaba's Qwen2.5 7b 16bit]
    #[serde(rename = "qwen2.5-coder:7b-instruct-fp16")]
    Qwen2_5coder7Bf16,
    /// [AliBaba's QwenQwq]
    #[serde(rename = "qwq")]
    QwenQwq,
    /// [DeepSeek Coding models]
    #[serde(rename = "deepseek-coder:6.7b")]
    DeepSeekCoder6_7B,
    /// [Mistral's MoE Models]
    #[serde(rename = "mixtral:8x7b")]
    Mixtral8_7b,
    /// [R1 Models]
    #[serde(rename = "deepseek-r1:1.5b")]
    R1_1_5b,
    #[serde(rename = "deepseek-r1:7b")]
    R1_7b,
    #[serde(rename = "deepseek-r1:8b")]
    R1_8b,
    #[serde(rename = "deepseek-r1:14b")]
    R1_14b,
    #[serde(rename = "deepseek-r1:32b")]
    R1_32b,
    #[serde(rename = "deepseek-r1:70b")]
    R1_70b,
    #[serde(rename = "deepseek-r1")]
    R1,
    #[serde(rename = "driaforall/tiny-agent-a:0.5b")]
    TinyAgent05,
    #[serde(rename = "driaforall/tiny-agent-a:1.5b")]
    TinyAgent15,
    #[serde(rename = "driaforall/tiny-agent-a:3b")]
    TinyAgent3,
    // OpenAI models
    /// [OpenAI's GPT-4 Turbo model](https://platform.openai.com/docs/models#gpt-4-turbo-and-gpt-4)
    #[serde(rename = "gpt-4-turbo")]
    GPT4Turbo,
    /// [OpenAI's GPT-4o model](https://platform.openai.com/docs/models#gpt-4o)
    #[serde(rename = "gpt-4o")]
    GPT4o,
    /// [OpenAI's GPT-4o mini model](https://platform.openai.com/docs/models#gpt-4o-mini)
    #[serde(rename = "gpt-4o-mini")]
    GPT4oMini,

    /// [OpenAI's o1 mini model](https://platform.openai.com/docs/models#o1)
    #[serde(rename = "o1-mini")]
    O1Mini,
    /// [OpenAI's o1 preview model](https://platform.openai.com/docs/models#o1)
    #[serde(rename = "o1-preview")]
    O1Preview,
    /// [OpenAI's o1 model](https://platform.openai.com/docs/models#o1)
    #[serde(rename = "o1")]
    O1,
    /// [OpenAI's o3 model](https://platform.openai.com/docs/models#o3-mini)
    #[serde(rename = "o3-mini")]
    O3Mini,

    // Gemini models
    /// Gemini 2.0 Pro exp model
    #[serde(rename = "gemini-2.0-pro-exp-02-05")]
    Gemini20Pro,
    /// Gemini 2.0 Flash exp model
    #[serde(rename = "gemini-2.0-flash")]
    Gemini20Flash,
    /// Gemini 1.5 Pro model
    #[serde(rename = "gemini-1.5-pro-exp-0827")]
    Gemini15ProExp0827,
    /// Gemini 1.5 Pro model
    #[serde(rename = "gemini-1.5-pro")]
    Gemini15Pro,
    /// Gemini 1.5 Flash model
    #[serde(rename = "gemini-1.5-flash")]
    Gemini15Flash,

    /// Gemma 2 2B IT model
    #[serde(rename = "gemma-2-2b-it")]
    Gemma2_2bIt,
    /// Gemma 2 9B IT model
    #[serde(rename = "gemma-2-9b-it")]
    Gemma2_9bIt,
    /// Gemma 2 27B IT model
    #[serde(rename = "gemma-2-27b-it")]
    Gemma2_27bIt,

    /// OpenRouter Models
    #[serde(rename = "meta-llama/llama-3.1-8b-instruct")]
    ORLlama3_1_8B,
    #[serde(rename = "meta-llama/llama-3.1-70b-instruct")]
    ORLlama3_1_70B,
    #[serde(rename = "meta-llama/llama-3.1-405b-instruct")]
    ORLlama3_1_405B,
    #[serde(rename = "meta-llama/llama-3.1-70b-instruct:free")]
    ORLlama3_1_70BFree,
    #[serde(rename = "meta-llama/llama-3.3-70b-instruct")]
    ORLlama3_3_70B,

    #[serde(rename = "anthropic/claude-3.5-sonnet:beta")]
    OR3_5Sonnet,
    #[serde(rename = "anthropic/claude-3-5-haiku-20241022:beta")]
    OR3_5Haiku,

    #[serde(rename = "qwen/qwen-2.5-72b-instruct")]
    ORQwen2_5_72B,
    #[serde(rename = "qwen/qwen-2.5-7b-instruct")]
    ORQwen2_5_7B,
    #[serde(rename = "qwen/qwen-2.5-coder-32b-instruct")]
    ORQwen2_5Coder32B,

    #[serde(rename = "eva-unit-01/eva-qwen-2.5-32b")]
    ORQwen2_5Eva32B,

    #[serde(rename = "qwen/qwq-32b-preview")]
    ORQwenQwq,

    #[serde(rename = "deepseek/deepseek-chat")]
    ORDeepSeek2_5,

    #[serde(rename = "nousresearch/hermes-3-llama-3.1-405b")]
    ORNousHermes405B,

    #[serde(rename = "nvidia/llama-3.1-nemotron-70b-instruct")]
    ORNemotron70B,

    #[serde(rename = "openai/o1")]
    OROpenAIO1,

    #[serde(rename = "deepseek/deepseek-r1-distill-llama-70b")]
    ORR1_70B,
    #[serde(rename = "deepseek/deepseek-r1")]
    ORR1,
}

impl Model {
    /// Returns an iterator over all models.
    #[inline(always)]
    pub fn all() -> impl Iterator<Item = Model> {
        enum_iterator::all::<Model>()
    }

    /// Returns an iterator over all models that belong to a given provider.
    #[inline(always)]
    pub fn all_with_provider(provider: &ModelProvider) -> impl Iterator<Item = Model> + '_ {
        enum_iterator::all::<Model>().filter(move |m| m.provider() == *provider)
    }

    /// Returns the provider that hosts the model.
    #[inline]
    pub fn provider(&self) -> ModelProvider {
        ModelProvider::from(self)
    }
}

impl From<Model> for String {
    fn from(model: Model) -> Self {
        model.to_string() // via Display
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // guaranteed not to fail because this is enum to string serialization
        let self_str = serde_json::to_string(&self).unwrap_or_default();

        // remove quotes from JSON
        write!(f, "{}", self_str.trim_matches('"'))
    }
}

impl TryFrom<String> for Model {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl TryFrom<&str> for Model {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // serde requires quotes (for JSON)
        serde_json::from_str::<Self>(&format!("\"{}\"", value))
            .map_err(|e| format!("Model {} invalid: {}", value, e))
    }
}

/// A model provider is a service that hosts the chosen Model.
/// It can be derived from the model name, e.g. GPT4o is hosted by OpenAI (via API), or Phi3 is hosted by Ollama (locally).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Sequence)]
pub enum ModelProvider {
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "openrouter")]
    OpenRouter,
}

impl ModelProvider {
    /// Returns an iterator over all model providers.
    #[inline(always)]
    pub fn all() -> impl Iterator<Item = ModelProvider> {
        enum_iterator::all::<ModelProvider>()
    }

    /// Returns all models that belong to the provider.
    #[inline]
    pub fn models(&self) -> impl Iterator<Item = Model> + '_ {
        Model::all_with_provider(self)
    }
}

impl From<Model> for ModelProvider {
    fn from(value: Model) -> Self {
        Self::from(&value)
    }
}

impl From<&Model> for ModelProvider {
    fn from(model: &Model) -> Self {
        match model {
            Model::NousTheta => ModelProvider::Ollama,
            Model::Phi3Medium => ModelProvider::Ollama,
            Model::Phi3Medium128k => ModelProvider::Ollama,
            Model::Phi3_5Mini => ModelProvider::Ollama,
            Model::Phi3_5MiniFp16 => ModelProvider::Ollama,
            Model::Llama3_1_8B => ModelProvider::Ollama,
            Model::Llama3_1_8Bq8 => ModelProvider::Ollama,
            Model::Llama3_1_8Bf16 => ModelProvider::Ollama,
            Model::Llama3_1_8BTextQ4KM => ModelProvider::Ollama,
            Model::Llama3_1_8BTextQ8 => ModelProvider::Ollama,
            Model::Llama3_1_70B => ModelProvider::Ollama,
            Model::Llama3_1_70Bq8 => ModelProvider::Ollama,
            Model::Llama3_1_70BTextQ4KM => ModelProvider::Ollama,
            Model::Llama3_2_1B => ModelProvider::Ollama,
            Model::Llama3_2_3B => ModelProvider::Ollama,
            Model::Llama3_2_1BTextQ4KM => ModelProvider::Ollama,
            Model::Llama3_3_70B => ModelProvider::Ollama,
            Model::Gemma2_9B => ModelProvider::Ollama,
            Model::Gemma2_9BFp16 => ModelProvider::Ollama,
            Model::Qwen2_5_7B => ModelProvider::Ollama,
            Model::Qwen2_5_7Bf16 => ModelProvider::Ollama,
            Model::Qwen2_5_32Bf16 => ModelProvider::Ollama,
            Model::Qwen2_5Coder1_5B => ModelProvider::Ollama,
            Model::Qwen2_5coder7B => ModelProvider::Ollama,
            Model::Qwen2_5oder7Bq8 => ModelProvider::Ollama,
            Model::Qwen2_5coder7Bf16 => ModelProvider::Ollama,
            Model::QwenQwq => ModelProvider::Ollama,
            Model::DeepSeekCoder6_7B => ModelProvider::Ollama,
            Model::Mixtral8_7b => ModelProvider::Ollama,
            Model::R1_1_5b => ModelProvider::Ollama,
            Model::R1_7b => ModelProvider::Ollama,
            Model::R1_8b => ModelProvider::Ollama,
            Model::R1_14b => ModelProvider::Ollama,
            Model::R1_32b => ModelProvider::Ollama,
            Model::R1_70b => ModelProvider::Ollama,
            Model::R1 => ModelProvider::Ollama,
            Model::TinyAgent05 => ModelProvider::Ollama,
            Model::TinyAgent15 => ModelProvider::Ollama,
            Model::TinyAgent3 => ModelProvider::Ollama,
            // openai
            Model::GPT4Turbo => ModelProvider::OpenAI,
            Model::GPT4o => ModelProvider::OpenAI,
            Model::GPT4oMini => ModelProvider::OpenAI,
            Model::O1Mini => ModelProvider::OpenAI,
            Model::O1Preview => ModelProvider::OpenAI,
            Model::O1 => ModelProvider::OpenAI,
            Model::O3Mini => ModelProvider::OpenAI,
            // gemini
            Model::Gemini20Flash => ModelProvider::Gemini,
            Model::Gemini20Pro => ModelProvider::Gemini,
            Model::Gemini15Flash => ModelProvider::Gemini,
            Model::Gemini15Pro => ModelProvider::Gemini,
            Model::Gemini15ProExp0827 => ModelProvider::Gemini,
            Model::Gemma2_2bIt => ModelProvider::Gemini,
            Model::Gemma2_9bIt => ModelProvider::Gemini,
            Model::Gemma2_27bIt => ModelProvider::Gemini,
            // openrouter
            Model::OR3_5Sonnet => ModelProvider::OpenRouter,
            Model::OR3_5Haiku => ModelProvider::OpenRouter,
            Model::ORDeepSeek2_5 => ModelProvider::OpenRouter,
            Model::ORLlama3_1_8B => ModelProvider::OpenRouter,
            Model::ORLlama3_1_70B => ModelProvider::OpenRouter,
            Model::ORLlama3_1_405B => ModelProvider::OpenRouter,
            Model::ORLlama3_1_70BFree => ModelProvider::OpenRouter,
            Model::ORLlama3_3_70B => ModelProvider::OpenRouter,
            Model::ORQwen2_5Coder32B => ModelProvider::OpenRouter,
            Model::ORQwen2_5_7B => ModelProvider::OpenRouter,
            Model::ORQwen2_5_72B => ModelProvider::OpenRouter,
            Model::ORQwen2_5Eva32B => ModelProvider::OpenRouter,
            Model::ORQwenQwq => ModelProvider::OpenRouter,
            Model::ORNemotron70B => ModelProvider::OpenRouter,
            Model::ORNousHermes405B => ModelProvider::OpenRouter,
            Model::OROpenAIO1 => ModelProvider::OpenRouter,
            Model::ORR1_70B => ModelProvider::OpenRouter,
            Model::ORR1 => ModelProvider::OpenRouter,
        }
    }
}

impl TryFrom<String> for ModelProvider {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        ModelProvider::try_from(value.as_str())
    }
}

impl TryFrom<&str> for ModelProvider {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // serde requires quotes (for JSON)
        serde_json::from_str::<Self>(&format!("\"{}\"", value))
            .map_err(|e| format!("Model provider {} invalid: {}", value, e))
    }
}

impl fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // guaranteed not to fail because this is enum to string serialization
        let self_str = serde_json::to_string(&self).unwrap_or_default();
        // remove quotes from JSON
        write!(f, "{}", self_str.trim_matches('"'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_string_conversion() {
        let model = Model::Phi3_5Mini;

        // convert to string
        let model_str: String = model.clone().into();
        assert_eq!(model_str, "phi3.5:3.8b");

        // (try) convert from string
        let model_from = Model::try_from(model_str).expect("should convert");
        assert_eq!(model_from, model);

        // (try) convert from string
        let model = Model::try_from("this-model-does-not-will-not-exist".to_string());
        assert!(model.is_err());
    }

    #[test]
    fn test_model_string_serde() {
        let model = Model::Phi3_5Mini;

        // serialize to string via serde
        let model_str = serde_json::to_string(&model).expect("should serialize");
        assert_eq!(model_str, "\"phi3.5:3.8b\"");

        // deserialize from string via serde
        let model_from: Model = serde_json::from_str(&model_str).expect("should deserialize");
        assert_eq!(model_from, model);

        // (try) deserialize from invalid model
        let bad_model = serde_json::from_str::<Model>("\"this-model-does-not-will-not-exist\"");
        assert!(bad_model.is_err());
    }

    #[test]
    fn test_provider_string_serde() {
        let provider = ModelProvider::OpenAI;

        // serialize to string via serde
        let provider_str = serde_json::to_string(&provider).expect("should serialize");
        assert_eq!(provider_str, "\"openai\"");

        // deserialize from string via serde
        let provider_from: ModelProvider =
            serde_json::from_str(&provider_str).expect("should deserialize");
        assert_eq!(provider_from, provider);

        // (try) deserialize from invalid model
        let bad_provider =
            serde_json::from_str::<ModelProvider>("\"this-provider-does-not-will-not-exist\"");
        assert!(bad_provider.is_err());
    }

    #[test]
    fn test_model_iterator() {
        let models = Model::all().collect::<Vec<_>>();
        assert!(models.len() > 20); // 20 is arbitrary but large enough
    }

    #[test]
    fn test_model_provider_iterator() {
        let models_providers = ModelProvider::all().collect::<Vec<_>>();
        assert!(models_providers.len() > 4); // 4 is arbitrary but large enough
    }
}
