use crate::shared::{ExecOptions, NodeInfo};
use crate::AsyncExecutable;
use futures::future::select_all;
use futures::FutureExt;
use petgraph::dot::Dot;
use petgraph::graph::NodeIndex;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

pub(crate) struct Task<Key: Hash + Eq + Clone, Node: AsyncExecutable> {
    pub node: Node,
    pub depends_on: HashSet<Key>,
    pub depended_on_by: HashSet<Key>,
    pub failed: bool,
    pub priority: usize,
}

pub struct AsyncGraphExecutor<Key: Hash + Eq + Clone, Node: AsyncExecutable> {
    pub(crate) tasks: HashMap<Key, Task<Key, Node>>,
    task_keys_with_no_deps: Vec<Key>,
    graph: petgraph::graph::DiGraph<Key, ()>,
    key_to_graph_idx: HashMap<Key, NodeIndex>,
}

#[derive(PartialEq, Eq)]
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
impl<Key: Eq + Hash + Clone + Sync + Send + Debug, Node: AsyncExecutable + Send>
    AsyncGraphExecutor<Key, Node>
{
    pub fn new(nodes: HashMap<Key, Node>, edges: Vec<(Key, Key)>) -> Self {
        let mut tasks: HashMap<Key, Task<Key, Node>> = nodes
            .into_iter()
            .map(|(key, node)| {
                (
                    key,
                    Task {
                        node,
                        depended_on_by: Default::default(),
                        depends_on: Default::default(),
                        failed: false,
                        priority: 0,
                    },
                )
            })
            .collect();

        log::debug!("make node_infos");
        edges.iter().for_each(|(subject_key, dependent_key)| {
            let subject_info = tasks.get_mut(subject_key).unwrap();
            subject_info.depended_on_by.insert(dependent_key.clone());
            let dependent_info = tasks.get_mut(dependent_key).unwrap();
            dependent_info.depends_on.insert(subject_key.clone());
        });
        log::debug!("make deps");

        let task_keys_with_no_deps = tasks
            .iter()
            .filter(|(_, node_info)| node_info.depends_on.len() == 0)
            .map(|(key, _)| key.clone())
            .collect();

        let mut graph: petgraph::graph::DiGraph<Key, ()> = Default::default();
        let mut key_to_graph_idx: HashMap<Key, NodeIndex> = Default::default();

        tasks.keys().for_each(|key| {
            if !key_to_graph_idx.contains_key(key) {
                let idx = graph.add_node(key.clone());
                key_to_graph_idx.insert(key.clone(), idx);
            }
        });

        edges.iter().for_each(|(from, to)| {
            let from_idx = key_to_graph_idx.get(from).unwrap();
            let to_idx = key_to_graph_idx.get(to).unwrap();
            graph.add_edge(*from_idx, *to_idx, ());
        });

        println!("{:#?}", Dot::new(&graph));

        Self {
            tasks,
            graph,
            // node_infos,
            task_keys_with_no_deps,
            key_to_graph_idx,
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
                value: key,
                order: 0,
            })
            .collect::<BinaryHeap<_>>();

        while running_futures.len() > 0 || todo_queue.len() > 0 {
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
                            order: 0,
                        })
                    }
                });

            self.tasks.insert(finished_task_key.value, finished_task);
        }
        Ok(())
    }
}
