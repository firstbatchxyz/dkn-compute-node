use dkn_workflows::{Entry, ExecutionError, Executor, Workflow};
use libsecp256k1::PublicKey;
use tokio::sync::mpsc;

use crate::payloads::TaskStats;

// TODO: instead of piggybacking stuff here, maybe node can hold it in a hashmap w.r.t taskId

pub struct WorkflowsWorkerInput {
    pub entry: Option<Entry>,
    pub executor: Executor,
    pub workflow: Workflow,
    pub task_id: String,
    // piggybacked
    pub public_key: PublicKey,
    pub model_name: String,
    pub stats: TaskStats,
    pub batchable: bool,
}

pub struct WorkflowsWorkerOutput {
    pub result: Result<String, ExecutionError>,
    pub task_id: String,
    // piggybacked
    pub public_key: PublicKey,
    pub model_name: String,
    pub stats: TaskStats,
    pub batchable: bool,
}

/// Workflows worker is a task executor that can process workflows in parallel / series.
///
/// It is expected to be spawned in another thread, with `run_batch` for batch processing and `run` for single processing.
pub struct WorkflowsWorker {
    /// Workflow message channel receiver, the sender is most likely the compute node itself.
    workflow_rx: mpsc::Receiver<WorkflowsWorkerInput>,
    /// Publish message channel sender, the receiver is most likely the compute node itself.
    publish_tx: mpsc::Sender<WorkflowsWorkerOutput>,
}

/// Buffer size for workflow tasks (per worker).
const WORKFLOW_CHANNEL_BUFSIZE: usize = 1024;

impl WorkflowsWorker {
    /// Batch size that defines how many tasks can be executed concurrently at once.
    ///
    /// The `run` function is designed to handle the batch size here specifically,
    /// if there are more tasks than the batch size, the function will panic.
    pub const MAX_BATCH_SIZE: usize = 8;

    /// Creates a worker and returns the sender and receiver for the worker.
    pub fn new(
        publish_tx: mpsc::Sender<WorkflowsWorkerOutput>,
    ) -> (WorkflowsWorker, mpsc::Sender<WorkflowsWorkerInput>) {
        let (workflow_tx, workflow_rx) = mpsc::channel(WORKFLOW_CHANNEL_BUFSIZE);

        let worker = WorkflowsWorker {
            workflow_rx,
            publish_tx,
        };

        (worker, workflow_tx)
    }

    /// Closes the workflow receiver channel.
    fn shutdown(&mut self) {
        log::info!("Closing workflows worker.");
        self.workflow_rx.close();
    }

    /// Launches the thread that can process tasks one by one (in series).
    /// This function will block until the channel is closed.
    ///
    /// It is suitable for task streams that consume local resources, unlike API calls.
    pub async fn run_series(&mut self) {
        loop {
            let task = self.workflow_rx.recv().await;

            if let Some(task) = task {
                log::info!("Processing single workflow for task {}", task.task_id);
                WorkflowsWorker::execute((task, &self.publish_tx)).await
            } else {
                return self.shutdown();
            };
        }
    }

    /// Launches the thread that can process tasks in batches.
    /// This function will block until the channel is closed.
    ///
    /// It is suitable for task streams that make use of API calls, unlike Ollama-like
    /// tasks that consumes local resources and would not make sense to run in parallel.
    ///
    /// Batch size must NOT be larger than `MAX_BATCH_SIZE`, otherwise will panic.
    pub async fn run_batch(&mut self, batch_size: usize) {
        assert!(
            batch_size <= Self::MAX_BATCH_SIZE,
            "Batch size must not be larger than {}",
            Self::MAX_BATCH_SIZE
        );

        loop {
            let mut tasks = Vec::new();

            // get tasks in batch from the channel, we enter the loop if:
            // (1) there are no tasks, or,
            // (2) there are tasks less than the batch size and the channel is not empty
            while tasks.is_empty() || (tasks.len() < batch_size && !self.workflow_rx.is_empty()) {
                log::info!(
                    "Worker is waiting for tasks ({} < {})",
                    tasks.len(),
                    batch_size
                );
                let limit = batch_size - tasks.len();
                match self.workflow_rx.recv_many(&mut tasks, limit).await {
                    // 0 tasks returned means that the channel is closed
                    0 => return self.shutdown(),
                    _ => {
                        // wait a small amount of time to allow for more tasks to be sent into the channel
                        tokio::time::sleep(std::time::Duration::from_millis(256)).await;
                    }
                }
            }

            // process the batch
            let num_tasks = tasks.len();
            debug_assert!(
                num_tasks <= batch_size,
                "number of tasks cant be larger than batch size"
            );
            debug_assert!(num_tasks != 0, "number of tasks cant be zero");
            log::info!("Processing {} workflows in batch", num_tasks);
            let mut batch = tasks.into_iter().map(|b| (b, &self.publish_tx));
            match num_tasks {
                1 => {
                    WorkflowsWorker::execute(batch.next().unwrap()).await;
                }
                2 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                3 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                4 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                5 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                6 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                7 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                8 => {
                    tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                }
                _ => {
                    unreachable!(
                        "number of tasks cant be larger than batch size ({} > {})",
                        num_tasks,
                        Self::MAX_BATCH_SIZE
                    );
                }
            };
        }
    }

    /// Executes a single task, and publishes the output.
    pub async fn execute(
        (mut input, publish_tx): (WorkflowsWorkerInput, &mpsc::Sender<WorkflowsWorkerOutput>),
    ) {
        input.stats = input.stats.record_execution_started_at();
        let result = input
            .executor
            .execute(
                input.entry.as_ref(),
                &input.workflow,
                &mut Default::default(),
            )
            .await;
        input.stats = input.stats.record_execution_ended_at();

        let output = WorkflowsWorkerOutput {
            result,
            public_key: input.public_key,
            task_id: input.task_id,
            model_name: input.model_name,
            batchable: input.batchable,
            stats: input.stats,
        };

        if let Err(e) = publish_tx.send(output).await {
            log::error!("Error sending workflow result: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use dkn_workflows::{Executor, Model};
    use libsecp256k1::{PublicKey, SecretKey};

    use super::*;
    use crate::payloads::TaskStats;

    /// Tests the workflows worker with a single task sent within a batch.
    ///
    /// ## Run command
    ///
    /// ```sh
    /// cargo test --package dkn-compute --lib --all-features -- workers::workflow::tests::test_workflows_worker --exact --show-output --nocapture --ignored
    /// ```
    #[tokio::test]
    #[ignore = "run manually"]
    async fn test_workflows_worker() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_compute", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (publish_tx, mut publish_rx) = mpsc::channel(1024);
        let (mut worker, workflow_tx) = WorkflowsWorker::new(publish_tx);

        // create batch workflow worker
        let worker_handle = tokio::spawn(async move {
            worker.run_batch(4).await;
        });

        let num_tasks = 4;
        let model = Model::O1Preview;
        let workflow = serde_json::json!({
            "config": {
                "max_steps": 10,
                "max_time": 250,
                "tools": [""]
            },
            "tasks": [
                {
                    "id": "A",
                    "name": "",
                    "description": "",
                    "operator": "generation",
                    "messages": [{ "role": "user", "content": "Write a 4 paragraph poem about Julius Caesar." }],
                    "inputs": [],
                    "outputs": [ { "type": "write", "key": "result", "value": "__result" } ]
                },
                {
                    "id": "__end",
                    "name": "end",
                    "description": "End of the task",
                    "operator": "end",
                    "messages": [{ "role": "user", "content": "End of the task" }],
                    "inputs": [],
                    "outputs": []
                }
            ],
            "steps": [ { "source": "A", "target": "__end" } ],
            "return_value": { "input": { "type": "read", "key": "result" }
            }
        });

        for i in 0..num_tasks {
            log::info!("Sending task {}", i + 1);

            let workflow = serde_json::from_value(workflow.clone()).unwrap();

            let executor = Executor::new(model.clone());
            let input = WorkflowsWorkerInput {
                entry: None,
                executor,
                workflow,
                public_key: PublicKey::from_secret_key(&SecretKey::default()),
                task_id: format!("task-{}", i + 1),
                model_name: model.to_string(),
                stats: TaskStats::default(),
                batchable: true,
            };

            // send workflow to worker
            workflow_tx.send(input).await.unwrap();
        }

        // now wait for all results
        let mut results = Vec::new();
        for i in 0..num_tasks {
            log::info!("Waiting for result {}", i + 1);
            let result = publish_rx.recv().await.unwrap();
            log::info!(
                "Got result {} (exeuction time: {})",
                i + 1,
                (result.stats.execution_ended_at - result.stats.execution_started_at) as f64
                    / 1_000_000_000f64
            );
            if result.result.is_err() {
                println!("Error: {:?}", result.result);
            }
            results.push(result);
        }

        log::info!("Got all results, closing channel.");
        publish_rx.close();

        // TODO: this bugs out
        worker_handle.await.unwrap();
        log::info!("Done.");
    }
}
