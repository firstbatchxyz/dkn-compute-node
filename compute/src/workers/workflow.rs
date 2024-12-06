use dkn_workflows::{Entry, ExecutionError, Executor, ProgramMemory, Workflow};
use libsecp256k1::PublicKey;
use tokio::sync::mpsc;

use crate::payloads::TaskStats;

// TODO: instead of piggybacking stuff here, maybe node can hold it in a hashmap w.r.t taskId

pub struct WorkflowsWorkerInput {
    pub entry: Option<Entry>,
    pub executor: Executor,
    pub workflow: Workflow,
    // piggybacked
    pub public_key: PublicKey,
    pub task_id: String,
    pub model_name: String,
    pub stats: TaskStats,
    pub batchable: bool,
}

pub struct WorkflowsWorkerOutput {
    pub result: Result<String, ExecutionError>,
    // piggybacked
    pub public_key: PublicKey,
    pub task_id: String,
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
                WorkflowsWorker::execute((task, self.publish_tx.clone())).await
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
        // TODO: need some better batch_size error handling here
        loop {
            // get tasks in batch from the channel
            let mut task_buffer = Vec::new();
            let num_tasks = self
                .workflow_rx
                .recv_many(&mut task_buffer, batch_size)
                .await;

            if num_tasks == 0 {
                return self.shutdown();
            }

            // process the batch
            log::info!("Processing {} workflows in batch", num_tasks);
            let mut batch = task_buffer
                .into_iter()
                .map(|b| (b, self.publish_tx.clone()));
            match num_tasks {
                1 => {
                    let r0 = WorkflowsWorker::execute(batch.next().unwrap()).await;
                    vec![r0]
                }
                2 => {
                    let (r0, r1) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1]
                }
                3 => {
                    let (r0, r1, r2) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1, r2]
                }
                4 => {
                    let (r0, r1, r2, r3) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1, r2, r3]
                }
                5 => {
                    let (r0, r1, r2, r3, r4) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1, r2, r3, r4]
                }
                6 => {
                    let (r0, r1, r2, r3, r4, r5) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1, r2, r3, r4, r5]
                }
                7 => {
                    let (r0, r1, r2, r3, r4, r5, r6) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1, r2, r3, r4, r5, r6]
                }
                8 => {
                    let (r0, r1, r2, r3, r4, r5, r6, r7) = tokio::join!(
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap()),
                        WorkflowsWorker::execute(batch.next().unwrap())
                    );
                    vec![r0, r1, r2, r3, r4, r5, r6, r7]
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
        (input, publish_tx): (WorkflowsWorkerInput, mpsc::Sender<WorkflowsWorkerOutput>),
    ) {
        let mut memory = ProgramMemory::new();

        let started_at = std::time::Instant::now();
        let result = input
            .executor
            .execute(input.entry.as_ref(), &input.workflow, &mut memory)
            .await;

        let output = WorkflowsWorkerOutput {
            result,
            public_key: input.public_key,
            task_id: input.task_id,
            model_name: input.model_name,
            batchable: input.batchable,
            stats: input.stats.record_execution_time(started_at),
        };

        if let Err(e) = publish_tx.send(output).await {
            log::error!("Error sending workflow result: {}", e);
        }
    }
}
