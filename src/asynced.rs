use crate::shared::{ExecOptions, NodeInfo};
use crate::AsyncExecutable;
use futures::future::select_all;
use futures::FutureExt;
use petgraph::dot::Dot;
use petgraph::graph::NodeIndex;
use std::collections::{BinaryHeap, HashMap};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug)]
pub struct AsyncGraphExecutor<Key: Hash + Eq + Clone, Node: AsyncExecutable> {
    node_infos: HashMap<Key, NodeInfo<Key>>,
    pub nodes: HashMap<Key, Node>,
    node_keys_with_no_deps: Vec<Key>,
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
        let mut node_infos = nodes
            .iter()
            .map(|(key, node)| {
                (
                    key.clone(),
                    NodeInfo::<Key> {
                        depended_on_by: Default::default(),
                        depends_on: Default::default(),
                        failed: false,
                        priority: node.get_priority(),
                    },
                )
            })
            .collect::<HashMap<Key, NodeInfo<Key>>>();

        log::debug!("make node_infos");
        edges.iter().for_each(|(subject_key, dependent_key)| {
            let subject_info = node_infos.get_mut(subject_key).unwrap();
            subject_info.depended_on_by.insert(dependent_key.clone());
            let dependent_info = node_infos.get_mut(dependent_key).unwrap();
            dependent_info.depends_on.insert(subject_key.clone());
        });
        log::debug!("make deps");

        let node_keys_with_no_deps = node_infos
            .iter()
            .filter(|(_, node_info)| node_info.depends_on.len() == 0)
            .map(|(key, _)| key.clone())
            .collect();

        let mut graph: petgraph::graph::DiGraph<Key, ()> = Default::default();
        let mut key_to_graph_idx: HashMap<Key, NodeIndex> = Default::default();

        nodes.keys().for_each(|key| {
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
            nodes,
            graph,
            node_infos,
            node_keys_with_no_deps,
            key_to_graph_idx,
        }
    }

    #[inline]
    pub async fn exec(self) {
        self.exec_with(Default::default()).await
    }

    pub async fn exec_with(mut self, options: ExecOptions) {
        // println!("nodeinfos {:#?}", self.node_infos);
        println!("start exec");
        let mut running_futures = vec![];
        let mut todo_queue = self
            .node_keys_with_no_deps
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
                let mut node = self.nodes.remove(&key.value).unwrap();
                running_futures.push(
                    async move {
                        let result = node.exec().await;
                        (key, result)
                    }
                    .boxed(),
                );
            }

            let ((finished_task_key, _result), idx, _remains) =
                select_all(&mut running_futures).await;
            running_futures.remove(idx);
            let info = self.node_infos.get_mut(&finished_task_key.value).unwrap();
            info.depended_on_by
                .clone()
                .into_iter()
                .for_each(|parent_key| {
                    let parent = self.node_infos.get_mut(&parent_key).unwrap();
                    parent.depends_on.remove(&finished_task_key.value);
                    if parent.depends_on.len() == 0 {
                        todo_queue.push(Ordered {
                            value: parent_key.clone(),
                            order: 0,
                        })
                    }
                });
        }
    }
}
