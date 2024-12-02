# Dria Workflows

We make use of [Ollama Workflows](https://github.com/andthattoo/ollama-workflows) in Dria Knowledge Network; however, we also want to make sure that the chosen models are valid and is performant enough (i.e. have enough TPS). This crate handles the configurations of models to be used, and implements various service checks.

There are two types of services:

- [`providers`](./src/providers/): these provide models that are directly used as `Model` enums in Workflows; they are only checked if a model that belongs to them is used.
- [`apis`](./src/apis/): these provide additional services used by workflows; they are only checked if their API key exists.

## Installation

Add the package via `git` within your Cargo dependencies:

```toml
dkn-workflows = { git = "https://github.com/firstbatchxyz/dkn-compute-node" }
```

Note that the underlying [Ollama Workflows](https://github.com/andthattoo/ollama-workflows) crate is re-exported by this crate.

## Usage

DKN Workflows make use of several environment variables, with respect to several model providers.

- `OLLAMA_HOST` is used to connect to **Ollama** server
- `OLLAMA_PORT` is used to connect to **Ollama** server
- `OLLAMA_AUTO_PULL` indicates whether we should pull missing models automatically or not
- `OPENAI_API_KEY` is used for **OpenAI** requests
- `GEMINI_API_KEY` is used for **Gemini** requests
- `SERPER_API_KEY` is optional API key to use **Serper**, for better Workflow executions
- `JINA_API_KEY` is optional API key to use **Jina**, for better Workflow executions

With the environment variables ready, you can simply create a new configuration and call `check_services` to ensure all models are correctly setup:

```rs
use dkn_workflows::{DriaWorkflowsConfig, Model};

let models = vec![Model::Phi3_5Mini];
let mut config = DriaWorkflowsConfig::new(models);
config.check_services().await?;
```
