use crate::shared::ExecOptions;
use crate::AsyncExecutable;
use futures::future::select_all;
use futures::FutureExt;
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug)]
pub(crate) struct Task<Key: Hash + Eq + Clone + Debug, Node: AsyncExecutable + Debug> {
    pub node: Node,
    pub depends_on: HashSet<Key>,
    pub depended_on_by: HashSet<Key>,
    pub failed: bool,
    pub priority: usize,
}

pub struct AsyncGraphExecutor<Key: Hash + Eq + Clone + Debug, Node: AsyncExecutable + Debug> {
    pub(crate) tasks: HashMap<Key, Task<Key, Node>>,
    task_keys_with_no_deps: Vec<Key>,
}

#[derive(PartialEq, Eq, Debug)]
struct Ordered<T> {
    value: T,
    order: usize,
}

impl<T: PartialEq> PartialOrd for Ordered<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.order.partial_cmp(&other.order)
    }
}

impl<T: Eq> Ord for Ordered<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order.cmp(&other.order)
    }
}
impl<Key: Eq + Hash + Clone + Sync + Send + Debug, Node: AsyncExecutable + Send + Debug>
    AsyncGraphExecutor<Key, Node>
{
    pub fn new(nodes: HashMap<Key, Node>, edges: Vec<(Key, Key)>) -> Self {
        let mut tasks: HashMap<Key, Task<Key, Node>> = nodes
            .into_iter()
            .map(|(key, node)| {
                (
                    key,
                    Task {
                        priority: 0,
                        node,
                        depended_on_by: Default::default(),
                        depends_on: Default::default(),
                        failed: false,
                    },
                )
            })
            .collect();

        edges.iter().for_each(|(subject_key, dependent_key)| {
            let subject_info = tasks.get_mut(subject_key).unwrap();
            subject_info.depended_on_by.insert(dependent_key.clone());
            let dependent_info = tasks.get_mut(dependent_key).unwrap();
            dependent_info.depends_on.insert(subject_key.clone());
        });

        let task_keys_with_no_deps = tasks
            .iter()
            .filter(|(_, node_info)| node_info.depends_on.len() == 0)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();

        let mut graph: petgraph::graph::DiGraph<&Key, ()> = Default::default();
        let mut key_to_graph_idx: HashMap<&Key, NodeIndex> = Default::default();

        tasks.keys().for_each(|key| {
            if !key_to_graph_idx.contains_key(key) {
                let idx = graph.add_node(&key);
                key_to_graph_idx.insert(&key, idx);
            }
        });
        edges.iter().for_each(|(from, to)| {
            graph.add_edge(
                *key_to_graph_idx.get(from).unwrap(),
                *key_to_graph_idx.get(to).unwrap(),
                (),
            );
        });

        let mut sorted = toposort(&graph, None).unwrap();

        while let Some(idx) = sorted.pop() {
            let max_p = graph
                .edges_directed(idx, EdgeDirection::Outgoing)
                .map(|edge| edge.target())
                .map(|idx| graph[idx])
                .map(|key| tasks.get(key).unwrap().node.get_priority())
                .max()
                .unwrap_or(0);
            let key = graph[idx];
            let task = tasks.get(key).unwrap();
            let p = max_p + task.node.get_priority();
            unsafe { (*(task as *const Task<Key, Node> as *mut Task<Key, Node>)).priority = p }
        }

        edges.iter().for_each(|(from, to)| {
            let from_idx = key_to_graph_idx.get(from).unwrap();
            let to_idx = key_to_graph_idx.get(to).unwrap();
            graph.add_edge(*from_idx, *to_idx, ());
        });

        Self {
            tasks,
            task_keys_with_no_deps,
        }
    }

    #[inline]
    pub async fn exec(&mut self) -> anyhow::Result<()> {
        self.exec_with(Default::default()).await
    }

    pub async fn exec_with(&mut self, options: ExecOptions) -> anyhow::Result<()> {
        assert!(options.concurrency > 0);

        let mut running_futures = vec![];

        let mut todo_queue = self
            .task_keys_with_no_deps
            .iter()
            .cloned()
            .map(|key| Ordered {
                order: self.tasks.get(&key).unwrap().priority,
                value: key,
            })
            .collect::<BinaryHeap<_>>();

        while running_futures.len() > 0 || todo_queue.len() > 0 {
            // println!("todo_queue {:#?}", todo_queue);
            while running_futures.len() < options.concurrency && todo_queue.len() > 0 {
                let key = todo_queue.pop().unwrap();
                let mut task = self.tasks.remove(&key.value).unwrap();
                running_futures.push(
                    async move {
                        if !task.failed {
                            let result = task.node.exec().await;
                            task.failed = result.is_err();
                        }
                        (key, task)
                    }
                    .boxed(),
                );
            }

            let ((finished_task_key, finished_task), idx, _remains) =
                select_all(&mut running_futures).await;
            running_futures.remove(idx);
            if finished_task.failed && !options.continue_on_fail {
                panic!("failed")
            }

            finished_task
                .depended_on_by
                .iter()
                .for_each(|consumer_key| {
                    let consumer = self.tasks.get_mut(&consumer_key).unwrap();
                    if finished_task.failed {
                        consumer.failed = finished_task.failed
                    }
                    consumer.depends_on.remove(&finished_task_key.value);
                    if consumer.depends_on.len() == 0 {
                        todo_queue.push(Ordered {
                            value: consumer_key.clone(),
                            order: consumer.priority,
                        })
                    }
                });

            self.tasks.insert(finished_task_key.value, finished_task);
        }
        Ok(())
    }
}
