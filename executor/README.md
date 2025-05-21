# Dria Executor

## Installation

Add the package via `git` within your Cargo dependencies:

```toml
dkn-executor = { git = "https://github.com/firstbatchxyz/dkn-compute-node" }
```

## Usage

Dria Executor makes use of several environment variables, with respect to several model providers.

- `OLLAMA_HOST` is used to connect to **Ollama** server
- `OLLAMA_PORT` is used to connect to **Ollama** server
- `OLLAMA_AUTO_PULL` indicates whether we should pull missing models automatically or not
- `OPENAI_API_KEY` is used for **OpenAI** requests
- `GEMINI_API_KEY` is used for **Gemini** requests
- `OPENROUTER_API_KEY` is used for **OpenRouter** requests.
