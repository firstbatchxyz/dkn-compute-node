use colored::Colorize;
use dkn_executor::{DriaExecutor, Model, TaskBody};
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::payloads::TaskStats;
use tokio::sync::mpsc;
use uuid::Uuid;

/// A metadata object that is kept aside while the worker is doing its job.
///
/// This is put into a map before execution, and then removed after the task is done.
pub struct TaskWorkerMetadata {
    pub model: Model,
    pub task_id: String,
    pub file_id: Uuid,
    /// If for any reason this object is dropped before `channel` is responded to,
    /// the task will be lost and the channel will be abruptly closed, causing an error on
    /// both the responder and the requester side, likely with an `OmissionError`.
    pub channel: ResponseChannel<Vec<u8>>,
}

pub struct TaskWorkerInput {
    /// used as identifier for metadata
    pub row_id: Uuid,
    // actual consumed input
    pub executor: DriaExecutor,
    pub task: TaskBody,
    // piggybacked metadata
    pub stats: TaskStats,
}

pub struct TaskWorkerOutput {
    // used as identifier for metadata
    pub row_id: Uuid,
    // actual produced output
    pub result: Result<String, dkn_executor::PromptError>,
    // piggybacked metadata
    pub stats: TaskStats,
    pub batchable: bool,
}

/// It is expected to be spawned in another thread, with [`Self::run_batch`] for batch processing and [`Self::run_series`] for single processing.
pub struct TaskWorker {
    /// Task channel receiver, the sender is most likely the compute node itself.
    task_rx: mpsc::Receiver<TaskWorkerInput>,
    /// Publish message channel sender, the receiver is most likely the compute node itself.
    publish_tx: mpsc::Sender<TaskWorkerOutput>,
    // TODO: batch size must be defined here
}

/// Buffer size for task channels (per worker).
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

    /// Closes the worker's receiver channel.
    fn shutdown(&mut self) {
        log::info!("Closing worker.");
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
                log::info!("Processing {} (single)", "task".yellow(),);
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
        let batchable = input.task.is_batchable();
        input.stats = input.stats.record_execution_started_at();
        let result = input.executor.execute(input.task).await;
        input.stats = input.stats.record_execution_ended_at();

        let output = TaskWorkerOutput {
            result,
            row_id: input.row_id,
            batchable,
            stats: input.stats,
        };

        if let Err(e) = publish_tx.send(output).await {
            log::error!("Error sending task result: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dkn_executor::{DriaExecutor, Model};

    /// Tests the worker with a single task sent within a batch.
    ///
    /// ## Run command
    ///
    /// ```sh
    /// cargo test --package dkn-compute --lib --all-features -- workers::task::tests::test_executor_worker --exact --show-output --nocapture --ignored
    /// ```
    #[tokio::test]
    #[ignore = "run manually"]
    async fn test_executor_worker() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_compute", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (publish_tx, mut publish_rx) = mpsc::channel(1024);
        let (mut worker, task_tx) = TaskWorker::new(publish_tx);

        // create batch worker
        let worker_handle = tokio::spawn(async move {
            worker.run_batch(4).await;
        });

        let num_tasks = 4;
        let model = Model::GPT4o;
        let executor = DriaExecutor::new_from_env(model.provider()).unwrap();
        let task = TaskBody::new_prompt("Write a poem about Julius Caesar.", model.clone());

        for i in 0..num_tasks {
            log::info!("Sending task {}", i + 1);

            let task_input = TaskWorkerInput {
                executor: executor.clone(),
                task: task.clone(),
                // dummy variables
                row_id: Uuid::now_v7(),
                stats: TaskStats::default(),
            };

            // send task to worker
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
