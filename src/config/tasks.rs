use std::env;

#[derive(Debug, Clone)]
pub struct DriaComputeNodeTasks {
    pub synthesis: bool,
    pub search: bool,
}

const TASK_SYNTHESIS: &str = "synthesis";
const TASK_SEARCH: &str = "search";

impl Default for DriaComputeNodeTasks {
    fn default() -> Self {
        Self {
            synthesis: true,
            search: true,
        }
    }
}

impl DriaComputeNodeTasks {
    pub fn new() -> Self {
        let tasks_str = env::var("DKN_TASKS").unwrap_or_default();
        Self::parse_str(tasks_str)
    }
    /// Parses a given string, expecting it to be a comma-separated list of task names, such as
    /// `synthesis,search`.
    pub fn parse_str(vec: String) -> Self {
        let mut synthesis = false;
        let mut search = false;

        let tasks: Vec<&str> = vec.split(',').collect();
        for task in tasks {
            match task.trim().to_lowercase().as_str() {
                TASK_SYNTHESIS => synthesis = true,
                TASK_SEARCH => search = true,
                _ => {
                    log::warn!("Unknown task: {}", task);
                }
            }
        }

        Self { synthesis, search }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsers() {
        env::set_var("DKN_TASKS", "fsfdshk,SynthEsis,fkdshfjsdk");
        let tasks = DriaComputeNodeTasks::new();
        assert!(tasks.synthesis);
        assert!(!tasks.search);

        env::set_var("DKN_TASKS", "fsfdshk, fdgsdg, search ");
        let tasks = DriaComputeNodeTasks::new();
        assert!(!tasks.synthesis);
        assert!(tasks.search);

        env::set_var("DKN_TASKS", ",,,");
        let tasks = DriaComputeNodeTasks::new();
        assert!(!tasks.synthesis);
        assert!(!tasks.search);
    }
}
