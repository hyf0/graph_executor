use std::collections::HashSet;

#[derive(Debug)]
pub(crate) struct NodeInfo<Key> {
    pub depends_on: HashSet<Key>,
    pub depended_on_by: HashSet<Key>,
    pub failed: bool,
    pub priority: usize,
}

pub struct ExecOptions {
    /// The maximum amount of promises that can be executing at the same time. When not provided, we do not limit the number of concurrent tasks and run tasks as soon as they are unblocked
    pub concurrency: usize,

    /// Continues the graph even if there's an rejected task.
    pub continue_on_fail: bool,
  }

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            concurrency: usize::MAX,
            continue_on_fail: true,
        }
    }
}
