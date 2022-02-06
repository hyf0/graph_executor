use crate::shared::NodeInfo;
use crate::AsyncExecutable;
use futures::future::select_all;
use futures::FutureExt;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug)]
pub struct AsyncGraphExecutor<Key: Hash + Eq + Clone, Node: AsyncExecutable> {
    node_infos: HashMap<Key, NodeInfo<Key>>,
    pub nodes: HashMap<Key, Node>,
    node_keys_with_no_deps: Vec<Key>,
}

impl<Key: Eq + Hash + Clone + Sync + Send + Debug, Node: AsyncExecutable + Send>
    AsyncGraphExecutor<Key, Node>
{
    pub fn new(nodes: HashMap<Key, Node>, edges: Vec<(Key, Key)>) -> Self {
        let mut node_infos = nodes
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
            .collect::<HashMap<_, _>>();
        log::debug!("make node_infos");
        edges.iter().for_each(|(subject_key, dependent_key)| {
            let subject_info = node_infos.get_mut(subject_key).unwrap();
            subject_info.depended_on_by.insert(dependent_key.clone());
            let dependent_info = node_infos.get_mut(dependent_key).unwrap();
            dependent_info.depends_on.insert(subject_key.clone());
            //  }
        });
        log::debug!("make deps");

        let node_keys_with_no_deps = node_infos
            .iter()
            .filter(|(_, node_info)| node_info.depends_on.len() == 0)
            .map(|(key, _)| key.clone())
            .collect();

        let me = Self {
            nodes,
            node_infos: node_infos,
            node_keys_with_no_deps,
        };
        me
    }

    pub async fn exec(&mut self) {
        // println!("nodeinfos {:#?}", self.node_infos);
        println!("start exec");
        let mut futures = vec![];
        {
            self.node_keys_with_no_deps.iter().for_each(|key| {
                let mut node = self.nodes.remove(&key).unwrap();
                futures.push(
                    async move {
                        let result = node.exec().await;
                        (key.clone(), result)
                    }
                    .boxed(),
                );
            });
        }
        while futures.len() > 0 {
            let ((finished_task_key, _result), idx, _remains) = select_all(&mut futures).await;
            futures.remove(idx);
            let info = self.node_infos.get_mut(&finished_task_key).unwrap();
            info.depended_on_by
                .clone()
                .into_iter()
                .for_each(|parent_key| {
                    let parent = self.node_infos.get_mut(&parent_key).unwrap();
                    parent.depends_on.remove(&finished_task_key);
                    if parent.depends_on.len() == 0 {
                        let mut node = self.nodes.remove(&parent_key).unwrap();
                        futures.push(
                            async move {
                                let result = node.exec().await;
                                (parent_key.clone(), result)
                            }
                            .boxed(),
                        );
                    }
                });
        }
    }
}
