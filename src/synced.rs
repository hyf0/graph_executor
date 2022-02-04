use dashmap::DashMap;
use priority_queue::PriorityQueue;
use std::collections::{HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::Executable;
use crate::shared::NodeInfo;

#[derive(Debug)]
pub struct GraphExecutor<Key: Hash + Eq + Clone, Node: Executable> {
    node_infos: Arc<DashMap<Key, NodeInfo<Key>>>,
    pub nodes: HashMap<Key, Node>,
    pub todo_queue: Arc<Mutex<PriorityQueue<Key, usize>>>,
    node_keys_with_no_deps: Vec<Key>,
}


impl<Key: Eq + Hash + Clone + Sync + Send + 'static, Node: Executable + Send + 'static>
    GraphExecutor<Key, Node>
{
    pub fn new(nodes: HashMap<Key, Node>, edges: Vec<(Key, Key)>) -> Self {
        let node_infos = nodes
            .iter()
            .map(|(key, _node)| {
                (
                    key.clone(),
                    NodeInfo::<Key> {
                        depended_on_by: Default::default(),
                        depends_on: Default::default(),
                        failed: false,
                        priority: 0,
                    },
                )
            })
            .collect::<DashMap<_, _>>();
        log::debug!("make node_infos");
        edges.iter().for_each(|(subject_key, dependent_key)| {
            let mut subject_info = node_infos.get_mut(subject_key).unwrap();
            subject_info.depended_on_by.insert(dependent_key.clone());
            let mut dependent_info = node_infos.get_mut(dependent_key).unwrap();
            dependent_info.depends_on.insert(subject_key.clone());
            //  }
        });
        log::debug!("make deps");

        let node_keys_with_no_deps = node_infos
            .iter()
            .filter(|node_info| node_info.depends_on.len() == 0)
            .map(|node_info| node_info.key().clone())
            .collect();

        let me = Self {
            nodes,
            node_infos: Arc::new(node_infos),
            node_keys_with_no_deps,
            // graph,
            todo_queue: Default::default(),
        };
        me
    }

    pub fn exec(&mut self) {
        log::debug!("start exec");
        {
            let mut todo_queue = self.todo_queue.lock().unwrap();
            self.node_keys_with_no_deps.iter().for_each(|key| {
                todo_queue.push(key.clone(), 0);
            });
        }

        let active_count = Arc::new(AtomicUsize::new(0));
        while active_count.load(Ordering::SeqCst) > 0 || !self.todo_queue.lock().unwrap().is_empty()
        {
            while let Some((key, _)) = self.todo_queue.lock().unwrap().pop() {
                let mut node = self.nodes.remove(&key).unwrap();
                let nodes_info = self.node_infos.clone();
                let todo_queue = self.todo_queue.clone();
                let active_count = active_count.clone();
                active_count.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    node.exec();
                    let info = nodes_info.get_mut(&key).unwrap();
                    info.depended_on_by.iter().for_each(|parent_key| {
                        let mut parent = nodes_info.get_mut(&parent_key).unwrap();
                        parent.depends_on.remove(&key);
                        if parent.depends_on.len() == 0 {
                            todo_queue.lock().unwrap().push(parent_key.clone(), 0);
                        }
                    });
                    active_count.fetch_sub(1, Ordering::SeqCst);
                });
            }
        }
    }
}