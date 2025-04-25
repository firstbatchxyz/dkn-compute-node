use colored::Colorize;
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::payloads::TaskStats;
use dkn_workflows::{DriaWorkflowsConfig, TaskBody, TaskResult};
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct TaskWorkerMetadata {
    pub model_name: String,
    pub channel: ResponseChannel<Vec<u8>>,
}

pub struct TaskWorkerInput {
    pub body: TaskBody,
    pub task_id: Uuid,
    pub row_id: Uuid,
    pub stats: TaskStats,
    pub batchable: bool,
}

pub struct TaskWorkerOutput {
    pub result: TaskResult,
    pub task_id: Uuid,
    pub row_id: Uuid,
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
    /// Workflows configuration.
    workflows: DriaWorkflowsConfig,
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
        workflows: DriaWorkflowsConfig,
    ) -> (TaskWorker, mpsc::Sender<TaskWorkerInput>) {
        let (task_tx, task_rx) = mpsc::channel(TASK_RX_CHANNEL_BUFSIZE);

        let worker = TaskWorker {
            workflows,
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
                self.execute((task, &self.publish_tx)).await
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
                    self.execute(batch.next().unwrap()).await;
                }
                2 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
                    );
                }
                3 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
                    );
                }
                4 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
                    );
                }
                5 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
                    );
                }
                6 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
                    );
                }
                7 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
                    );
                }
                8 => {
                    tokio::join!(
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap()),
                        self.execute(batch.next().unwrap())
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
        &self,
        (mut input, publish_tx): (TaskWorkerInput, &mpsc::Sender<TaskWorkerOutput>),
    ) {
        input.stats = input.stats.record_execution_started_at();
        let result = self.workflows.execute(input.body).await;
        input.stats = input.stats.record_execution_ended_at();

        let output = TaskWorkerOutput {
            result,
            task_id: input.task_id,
            row_id: input.row_id,
            batchable: input.batchable,
            stats: input.stats,
        };

        if let Err(e) = publish_tx.send(output).await {
            log::error!("Error sending workflow result: {}", e);
        }
    }
}
