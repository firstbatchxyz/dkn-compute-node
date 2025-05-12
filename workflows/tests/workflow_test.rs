use dkn_workflows::{Executor, Model, Workflow};

#[tokio::test]
#[ignore = "requires API keys"]
async fn test_workflow() {
    dotenvy::dotenv().ok(); // read api key
    let workflow = serde_json::from_value::<Workflow>(serde_json::json!({
      "steps": [{ "source": "A", "target": "__end" }],
      "tasks": [
        {
          "id": "A",
          "name": "",
          "outputs": [
            {
              "key": "result",
              "type": "write",
              "value": "__result"
            }
          ],
          "messages": [
            {
              "role": "user",
              "content": "Hey there."
            }
          ],
          "operator": "generation",
          "description": ""
        },
        {
          "id": "__end",
          "name": "end",
          "messages": [
            {
              "role": "user",
              "content": "End of the task"
            }
          ],
          "operator": "end",
          "description": "End of the task"
        }
      ],
      "config": {
        "tools": [
          "ALL"
        ],
        "max_time": 250,
        "max_steps": 10
      },
      "return_value": {
        "input": {
          "key": "result",
          "type": "read"
        }
      }
    }))
    .unwrap();

    let executor = Executor::new(Model::GPT4oMini);
    let result = executor
        .execute(None, &workflow, &mut Default::default())
        .await
        .unwrap();
    println!("Result: {:?}", result);
}
