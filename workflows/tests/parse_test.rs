use dkn_workflows::Workflow;

#[test]
fn test_parse_example() {
    let object = serde_json::json!({
       "config":{
          "max_steps":50,
          "max_time":50,
          "tools":[
             "ALL"
          ],
          "custom_tools":null,
          "max_tokens":null
       },
       "external_memory":{
          "topic":"Machine Learning",
          "n_subtopics":"10"
       },
       "tasks":[
          {
             "id":"generate_subtopics",
             "name":"Task",
             "description":"Task Description",
             "messages":[
                {
                   "role":"user",
                   "content":"Given a topic, generate a list of {{n_subtopics}} subtopics that are related to the topic.\nThe topic is: {{topic}}\nThe list must be without numbers, and without any description of the subtopics. \nThe subtopics should be separated by a comma. There must be no other text than the list.\n"
                }
             ],
             "schema":null,
             "inputs":[
                {
                   "name":"n_subtopics",
                   "value":{
                      "type":"read",
                      "index":null,
                      "search_query":null,
                      "key":"n_subtopics"
                   },
                   "required":true
                },
                {
                   "name":"topic",
                   "value":{
                      "type":"read",
                      "index":null,
                      "search_query":null,
                      "key":"topic"
                   },
                   "required":true
                }
             ],
             "operator":"generation",
             "outputs":[
                {
                   "type":"write",
                   "key":"subtopics",
                   "value":"__result"
                }
             ]
          },
          {
             "id":"_end",
             "name":"Task",
             "description":"Task Description",
             "messages":[
                {
                   "role":"user",
                   "content":""
                }
             ],
             "schema":null,
             "operator":"end",
          }
       ],
       "steps":[
          {
             "source":"generate_subtopics",
             "target":"_end",
             "condition":null,
             "fallback":null
          }
       ],
       "return_value":{
          "input":{
             "type":"read",
             "index":null,
             "search_query":null,
             "key":"subtopics"
          },
          "to_json":false,
          "post_process":null
       }
    });

    assert!(
        serde_json::from_value::<Workflow>(object).is_ok(),
        "could not parse"
    );
}
