use std::collections::HashSet;

#[derive(Debug)]
pub(crate) struct NodeInfo<Key> {
    pub depends_on: HashSet<Key>,
    pub depended_on_by: HashSet<Key>,
    pub failed: bool,
    pub priority: usize,
}

pub struct ExecOptions {
    pub concurrency: usize,
}

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            concurrency: usize::MAX,
        }
    }
}
