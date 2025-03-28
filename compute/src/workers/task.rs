use colored::Colorize;
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_workflows::{Entry, ExecutionError, Executor, Workflow};
use libsecp256k1::PublicKey;
use tokio::sync::mpsc;

use crate::payloads::TaskStats;

pub struct TaskWorkerMetadata {
    pub public_key: PublicKey,
    pub model_name: String,
    pub channel: ResponseChannel<Vec<u8>>,
}

pub struct TaskWorkerInput {
    pub entry: Option<Entry>,
    pub executor: Executor,
    pub workflow: Workflow,
    pub task_id: String,
    pub stats: TaskStats,
    pub batchable: bool,
}

pub struct TaskWorkerOutput {
    pub result: Result<String, ExecutionError>,
    pub task_id: String,
    pub stats: TaskStats,
    pub batchable: bool,
}

/// Workflows worker is a task executor that can process workflows in parallel / series.
///
/// It is expected to be spawned in another thread, with `run_batch` for batch processing and `run` for single processing.
pub struct TaskWorker {
    /// Workflow message channel receiver, the sender is most likely the compute node itself.
    task_rx: mpsc::Receiver<TaskWorkerInput>,
    /// Publish message channel sender, the receiver is most likely the compute node itself.
    publish_tx: mpsc::Sender<TaskWorkerOutput>,
}

/// Buffer size for workflow tasks (per worker).
const TASK_RX_CHANNEL_BUFSIZE: usize = 1024;

impl TaskWorker {
    /// Batch size that defines how many tasks can be executed concurrently at once.
    ///
    /// The `run` function is designed to handle the batch size here specifically,
    /// if there are more tasks than the batch size, the function will panic.
    pub const MAX_BATCH_SIZE: usize = 8;

    /// Creates a worker and returns the sender and receiver for the worker.
    pub fn new(
        publish_tx: mpsc::Sender<TaskWorkerOutput>,
    ) -> (TaskWorker, mpsc::Sender<TaskWorkerInput>) {
        let (task_tx, task_rx) = mpsc::channel(TASK_RX_CHANNEL_BUFSIZE);

        let worker = TaskWorker {
            task_rx,
            publish_tx,
        };

        (worker, task_tx)
    }

    /// Closes the workflow receiver channel.
    fn shutdown(&mut self) {
        log::info!("Closing workflows worker.");
        self.task_rx.close();
    }

    /// Launches the thread that can process tasks one by one (in series).
    /// This function will block until the channel is closed.
    ///
    /// It is suitable for task streams that consume local resources, unlike API calls.
    pub async fn run_series(&mut self) {
        loop {
            let task = self.task_rx.recv().await;

            if let Some(task) = task {
                log::info!("Processing {} {} (single)", "task".yellow(), task.task_id);
                TaskWorker::execute((task, &self.publish_tx)).await
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
            while tasks.is_empty() || (tasks.len() < batch_size && !self.task_rx.is_empty()) {
                log::info!(
                    "Worker is waiting for tasks ({} < {})",
                    tasks.len(),
                    batch_size
                );
                let limit = batch_size - tasks.len();
                match self.task_rx.recv_many(&mut tasks, limit).await {
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

            log::info!("Processing {} tasks in batch", num_tasks);
            let mut batch = tasks.into_iter().map(|b| (b, &self.publish_tx));
            match num_tasks {
                1 => {
                    TaskWorker::execute(batch.next().unwrap()).await;
                }
                2 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
                    );
                }
                3 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
                    );
                }
                4 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
                    );
                }
                5 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
                    );
                }
                6 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
                    );
                }
                7 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
                    );
                }
                8 => {
                    tokio::join!(
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap()),
                        TaskWorker::execute(batch.next().unwrap())
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
        (mut input, publish_tx): (TaskWorkerInput, &mpsc::Sender<TaskWorkerOutput>),
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

        let output = TaskWorkerOutput {
            result,
            task_id: input.task_id,
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
        let (mut worker, task_tx) = TaskWorker::new(publish_tx);

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
                    "outputs": [ { "type": "write", "key": "result", "value": "__result" } ]
                },
                {
                    "id": "__end",
                    "name": "end",
                    "description": "End of the task",
                    "operator": "end",
                    "messages": [{ "role": "user", "content": "End of the task" }],
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
            let task_input = TaskWorkerInput {
                entry: None,
                executor,
                workflow,
                task_id: format!("task-{}", i + 1),
                stats: TaskStats::default(),
                batchable: true,
            };

            // send workflow to worker
            task_tx.send(task_input).await.unwrap();
        }

        // now wait for all results
        let mut results = Vec::new();
        for i in 0..num_tasks {
            log::info!("Waiting for result {}", i + 1);
            let result = publish_rx.recv().await.unwrap();
            log::info!("Got result {}", i + 1,);
            if result.result.is_err() {
                log::error!("Error: {:?}", result.result);
            }
            results.push(result);
        }

        log::info!("Got all results, closing channel.");
        publish_rx.close();

        // FIXME: this bugs out
        worker_handle.await.unwrap();
        log::info!("Done.");
    }
}
