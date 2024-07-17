#![allow(unused_imports)]

use langchain_rust::{language_models::llm::LLM, llm::client::Ollama};
use ollama_workflows::{Entry, Executor, Model, ProgramMemory, Workflow};
use std::env;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "run this manually"]
async fn test_ollama_prompt() {
    let model = "orca-mini".to_string();
    let ollama = Ollama::default().with_model(model);
    let prompt = "The sky appears blue during the day because of a process called scattering. \
                When sunlight enters the Earth's atmosphere, it collides with air molecules such as oxygen and nitrogen. \
                These collisions cause some of the light to be absorbed or reflected, which makes the colors we see appear more vivid and vibrant. \
                Blue is one of the brightest colors that is scattered the most by the atmosphere, making it visible to our eyes during the day. \
                What may be the question this answer?".to_string();

    let response = ollama
        .invoke(&prompt)
        .await
        .expect("Should generate response");
    println!("Prompt: {}\n\nResponse:{}", prompt, response);
}

#[tokio::test]
#[ignore = "run this manually"]
async fn test_ollama_bad_model() {
    let model = "thismodeldoesnotexistlol".to_string();
    let ollama = Ollama::default().with_model(model);
    let setup_res = ollama.invoke("hola").await;
    assert!(
        setup_res.is_err(),
        "Should give error due to non-existing model."
    );
}

#[tokio::test]
#[ignore = "run this manually"]
async fn test_ollama_workflow() {
    let workflow = r#"{
    "name": "Simple",
    "description": "This is a simple workflow",
    "config": {
        "max_steps": 5,
        "max_time": 100,
    },
    "tasks":[
        {
            "id": "A",
            "name": "Random Poem",
            "description": "Writes a poem about Kapadokya.",
            "prompt": "Please write a poem about Kapadokya.",
            "operator": "generation",
            "outputs": [
                {
                    "type": "write",
                    "key": "final_result",
                    "value": "__result"
                }
            ]
        },
        {
            "id": "__end",
            "name": "end",
            "description": "End of the task",
            "prompt": "End of the task",
            "operator": "end",
        }
    ],
    "steps":[
        {
            "source":"A",
            "target":"end"
        }
    ]
}"#;
    let workflow: Workflow = serde_json::from_str(workflow).unwrap();
    let exe = Executor::new(Model::Phi3Mini);
    let mut memory = ProgramMemory::new();

    let result = exe.execute(None, workflow, &mut memory).await;
    println!("Result: {}", result);
}
