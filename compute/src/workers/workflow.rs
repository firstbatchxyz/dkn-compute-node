use dkn_workflows::{Entry, ExecutionError, Executor, ProgramMemory, Workflow};
use libsecp256k1::PublicKey;
use tokio::sync::mpsc;

use crate::payloads::TaskStats;

pub struct WorkflowsWorkerInput {
    pub entry: Option<Entry>,
    pub executor: Executor,
    pub workflow: Workflow,
    // piggybacked
    pub public_key: PublicKey,
    pub task_id: String,
    pub model_name: String,
    pub stats: TaskStats,
}

pub struct WorkflowsWorkerOutput {
    pub result: Result<String, ExecutionError>,
    // piggybacked
    pub public_key: PublicKey,
    pub task_id: String,
    pub model_name: String,
    pub stats: TaskStats,
}

pub struct WorkflowsWorker {
    worklow_rx: mpsc::Receiver<WorkflowsWorkerInput>,
    publish_tx: mpsc::Sender<WorkflowsWorkerOutput>,
}

impl WorkflowsWorker {
    /// Batch size that defines how many tasks can be executed in parallel at once.
    /// IMPORTANT NOTE: `run` function is designed to handle the batch size here specifically,
    /// if there are more tasks than the batch size, the function will panic.
    const BATCH_SIZE: usize = 5;

    pub fn new(
        worklow_rx: mpsc::Receiver<WorkflowsWorkerInput>,
        publish_tx: mpsc::Sender<WorkflowsWorkerOutput>,
    ) -> Self {
        Self {
            worklow_rx,
            publish_tx,
        }
    }

    pub async fn run(&mut self) {
        loop {
            // get tasks in batch from the channel
            let mut batch_vec = Vec::new();
            let num_tasks = self
                .worklow_rx
                .recv_many(&mut batch_vec, Self::BATCH_SIZE)
                .await;
            debug_assert!(
                num_tasks <= Self::BATCH_SIZE,
                "drain cant be larger than batch size"
            );
            // TODO: just to be sure, can be removed later
            debug_assert_eq!(num_tasks, batch_vec.len());

            if num_tasks == 0 {
                self.worklow_rx.close();
                return;
            }

            // process the batch
            let mut batch = batch_vec.into_iter();
            log::info!("Processing {} workflows in batch", num_tasks);
            let results = match num_tasks {
                1 => vec![WorkflowsWorker::execute(batch.next().unwrap()).await],
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
                _ => {
                    unreachable!("drain cant be larger than batch size");
                }
            };

            // publish all results
            // TODO: make this a part of executor as well
            log::info!("Publishing {} workflow results", results.len());
            for result in results {
                if let Err(e) = self.publish_tx.send(result).await {
                    log::error!("Error sending workflow result: {}", e);
                }
            }
        }
    }

    /// A single task execution.
    pub async fn execute(input: WorkflowsWorkerInput) -> WorkflowsWorkerOutput {
        let mut memory = ProgramMemory::new();

        let started_at = std::time::Instant::now();
        let result = input
            .executor
            .execute(input.entry.as_ref(), &input.workflow, &mut memory)
            .await;

        WorkflowsWorkerOutput {
            result,
            public_key: input.public_key,
            task_id: input.task_id,
            model_name: input.model_name,
            stats: input.stats.record_execution_time(started_at),
        }
    }
}
