#[cfg(test)]
mod threads_test {
    use parking_lot::RwLock;
    use std::sync::Arc;
    use tokio_util::task::TaskTracker;

    struct BusyStruct {
        pub busy_lock: RwLock<bool>,
    }

    impl BusyStruct {
        pub fn new() -> Self {
            Self {
                busy_lock: RwLock::new(false),
            }
        }
        /// Returns the state of the node, whether it is busy or not.
        #[inline]
        pub fn is_busy(&self) -> bool {
            *self.busy_lock.read()
        }

        /// Set the state of the node, whether it is busy or not.
        #[inline]
        pub fn set_busy(&self, busy: bool) {
            log::info!("Setting busy to {}", busy);
            *self.busy_lock.write() = busy;
        }
    }

    /// This test demonstrates that two threads dont wait for each other.
    /// We need a separate busy lock for task types, so that heartbeat messages can be
    /// repsonded to with the correct task types.
    /// Run with:
    ///
    /// ```sh
    /// cargo test --package dkn-compute --test threads_test --all-features -- threads_test::test_mutex --exact --show-output
    /// ```
    #[tokio::test]
    #[ignore = "only run this for demonstration"]
    async fn test_mutex() {
        let _ = env_logger::try_init();
        let tracker = TaskTracker::new();
        let obj = Arc::new(BusyStruct::new());

        println!("Starting test");

        // spawn a thread
        let obj1 = obj.clone();
        tracker.spawn(tokio::spawn(async move {
            println!("Thread 1 | is_busy: {}", obj1.is_busy());
            println!("Thread 1 | Started");
            obj1.set_busy(true);

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            obj1.set_busy(false);
            println!("Thread 1 | Finished");
        }));

        // wait a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        // spawn a thread
        let obj2 = obj.clone();
        tracker.spawn(tokio::spawn(async move {
            println!("Thread 2 | is_busy: {}", obj2.is_busy());
            println!("Thread 2 | Started");
            obj2.set_busy(true);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            obj2.set_busy(false);
            println!("Thread 2 | Finished");
        }));

        tracker.close();
        println!("Waiting...");
        tracker.wait().await;

        println!("Done.");
    }
}
