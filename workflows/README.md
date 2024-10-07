# DKN Workflows

We make use of Ollama Workflows in DKN; however, we also want to make sure that the chosen models are valid and is performant enough (i.e. have enough TPS).
This crate handles the configurations of models to be used, and implements various service checks.

- **OpenAI**: We check that the chosen models are enabled for the user's profile by fetching their models with their API key. We filter out the disabled models.
- **Ollama**: We provide a sample workflow to measure TPS and then pick models that are above some TPS threshold. While calculating TPS, there is also a timeout so that beyond that timeout the TPS is not even considered and the model becomes invalid.

## Installation

TODO: !!!

## Usage

DKN Workflows make use of several environment variables, respecting the providers.

- `OPENAI_API_KEY` is used for OpenAI requests
- `OLLAMA_HOST` is used to connect to Ollama server
- `OLLAMA_PORT` is used to connect to Ollama server
- `OLLAMA_AUTO_PULL` indicates whether we should pull missing models automatically or not
- `SERPER_API_KEY` is optional API key to use **Serper**, for better Workflow executions
- `JINA_API_KEY` is optional API key to use **Jina**, for better Workflow executions

With the environment variables ready, you can simply create a new configuration and call `check_services` to ensure all models are correctly setup:

```rs
use dkn_workflows::{DriaWorkflowsConfig, Model};

let models = vec![Model::Phi3_5Mini];
let mut config = DriaWorkflowsConfig::new(models);
config.check_services().await?;
```
