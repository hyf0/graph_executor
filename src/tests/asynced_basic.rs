use anyhow::bail;
use async_trait::async_trait;

use core::fmt;
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{AsyncExecutable, AsyncGraphExecutor, ExecOptions};

#[derive(Debug, Default, Clone)]
struct TestNode {
    pub id: &'static str,
    records: Arc<Mutex<CallRecords>>,
    rejected: bool,
    called: bool,
}

impl TestNode {
    pub fn new(id: &'static str, records: Arc<Mutex<CallRecords>>, reject: bool) -> Self {
        Self {
            id,
            records,
            rejected: reject,
            called: false,
        }
    }
}

#[derive(Debug)]
struct SuperError;

impl fmt::Display for SuperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SuperError is here!")
    }
}

impl Error for SuperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[async_trait]
impl AsyncExecutable for TestNode {
    async fn exec(&mut self) -> anyhow::Result<()> {
        assert!(!self.called);
        let records = self.records.clone();
        records.lock().unwrap().lived_call += 1;
        let lived_call = records.lock().unwrap().lived_call;
        let max_call = records.lock().unwrap().max_call;
        records.lock().unwrap().max_call = if max_call > lived_call {
            max_call
        } else {
            lived_call
        };
        self.called = true;
        if self.rejected {
            records.lock().unwrap().lived_call -= 1;
            bail!(SuperError)
        } else {
            println!("exec {}", self.id);
            tokio::time::sleep(Duration::from_millis(100)).await;
            // std::thread::sleep();
            records.lock().unwrap().exec_records.push(self.id);
            records.lock().unwrap().lived_call -= 1;
            Ok(())
        }
    }
}

#[derive(Debug, Default, Clone)]
struct CallRecords {
    pub max_call: usize,
    pub lived_call: usize,
    pub exec_records: Vec<&'static str>,
}

#[derive(Default)]
struct TestSuite {
    pub records: Arc<Mutex<CallRecords>>,
    pub nodes: Vec<TestNode>,
    pub edges: Vec<(&'static str, &'static str)>,
    pub executor: Option<AsyncGraphExecutor<&'static str, TestNode>>,
}

impl TestSuite {
    pub fn build() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, id: &'static str) -> &mut Self {
        self.nodes
            .push(TestNode::new(id, self.records.clone(), false));
        self
    }

    pub fn add_nodes(&mut self, mut ids: Vec<&'static str>) -> &mut Self {
        self.nodes.append(
            &mut ids
                .into_iter()
                .map(|id| TestNode::new(id, self.records.clone(), false))
                .collect(),
        );
        self
    }
    pub fn add_rejected_node(&mut self, id: &'static str) -> &mut Self {
        self.nodes
            .push(TestNode::new(id, self.records.clone(), true));
        self
    }
    
    // pub fn add_edge(&mut self, edge: (&'static str, &'static str)) -> &mut Self {
    //     self.edges.push(edge);
    //     self
    // }

    pub fn add_edges(&mut self, mut edges: Vec<(&'static str, &'static str)>) -> &mut Self {
        self.edges.append(&mut edges);
        self
    }

    pub fn gen_graph_runner(&mut self) -> &mut AsyncGraphExecutor<&'static str, TestNode> {
        let nodes = self
            .nodes
            .iter()
            .cloned()
            .map(|s| (s.id, s))
            .collect::<HashMap<_, _>>();
        let edges = self.edges.clone();
        let executor = AsyncGraphExecutor::new(nodes, edges);
        self.executor = Some(executor);
        self.executor.as_mut().unwrap()
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.gen_graph_runner().exec().await
    }

    pub async fn run_with(&mut self, ops: ExecOptions) -> anyhow::Result<()> {
        self.gen_graph_runner().exec_with(ops).await
    }

    pub fn node(&self, id: &'static str) -> &TestNode {
        &self.executor.as_ref().unwrap().tasks.get(id).unwrap().node
    }
}

#[test]
fn basic() {
    //       putOnShorts         putOnShirt
    //       /         \          /
    //  putOnShoes      putOnJacket
    //           \
    //            tieShoes
    let mut t = TestSuite::build();
    let f = t
        .add_node("putOnShirt")
        .add_node("putOnShorts")
        .add_node("putOnJacket")
        .add_node("putOnShoes")
        .add_node("tieShoes")
        .add_edges(vec![
            ("putOnShirt", "putOnJacket"),
            ("putOnShoes", "tieShoes"),
            ("putOnShorts", "putOnJacket"),
            ("putOnShorts", "putOnShoes"),
        ])
        .run();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();

    assert_ordering(
        &t.records.lock().unwrap().exec_records,
        "putOnShoes",
        "tieShoes",
    );
    assert_ordering(
        &t.records.lock().unwrap().exec_records,
        "putOnShirt",
        "putOnJacket",
    );
    assert_ordering(
        &t.records.lock().unwrap().exec_records,
        "putOnShorts",
        "putOnJacket",
    );
    assert_ordering(
        &t.records.lock().unwrap().exec_records,
        "putOnShorts",
        "putOnShoes",
    );
}

fn throws_an_exception_when_the_dependency_graph_has_a_cycle_starting_from_the_root() {}

fn throws_an_exception_with_detailed_message_when_the_dependency_graph_has_a_cycle() {}

fn throws_an_exception_in_the_first_instance_of_a_cycle_that_has_been_detected_when_there_are_overlapped_cycles(
) {
}

#[test]
fn resolves_an_empty_dependnecy_graph() {
    let mut t = TestSuite::build();
    let f = t.run();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();
    assert!(t.records.lock().unwrap().exec_records.is_empty())
}
#[test]
fn throws_an_exception_but_continues_to_run_the_entire_graph() {
    let mut t = TestSuite::build();
    let f = t
        .add_nodes(vec!["A", "B", "D", "E", "F"])
        .add_rejected_node("C")
        .add_edges(vec![
            ("A", "B"),
            ("A", "C"),
            ("A", "D"),
            ("C", "D"),
            ("A", "E"),
            ("E", "F"),
        ])
        .run();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();

    assert!(t.node("A").called);
    assert!(t.node("B").called);
    assert!(t.node("E").called);
    assert!(t.node("F").called);
    assert!(t.node("C").called);
    assert!(!t.node("D").called);
}

#[test]
#[should_panic]
fn throws_when_one_of_the_dependencies_references_a_node_not_in_the_node_map() {
    let mut t = TestSuite::build();
    let f = t
        .add_nodes(vec!["A", "B"])
        .add_edges(vec![("A", "B"), ("A", "C"), ("E", "F")])
        .run();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();
}

#[test]
fn should_run_all_dependencies_for_disconnected_graphs() {
    let mut t = TestSuite::build();
    let f = t
        .add_nodes(vec!["A", "B", "C", "D"])
        .add_edges(vec![("A", "B"), ("A", "C")])
        .run();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();

    assert!(t.node("A").called);
    assert!(t.node("B").called);
    assert!(t.node("C").called);
    assert!(t.node("D").called);
}

#[test]
fn should_be_able_to_run_more_than_one_task_at_a_time() {
    let mut t = TestSuite::build();
    let f = t
        .add_nodes(vec!["A", "B", "C"])
        .add_edges(vec![("A", "B"), ("A", "C")])
        .run();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();
    assert_eq!(t.records.lock().unwrap().max_call, 2);
}

#[test]
fn should_not_exceed_maximum_concurrency() {
    let mut t = TestSuite::build();
    let f = t
        .add_nodes(vec!["A", "B", "C", "D", "E"])
        .add_edges(vec![("A", "B"), ("A", "C"), ("A", "D"), ("A", "E")])
        .run_with(ExecOptions {
            concurrency: 3,
            ..Default::default()
        });
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();
    assert_eq!(t.records.lock().unwrap().max_call, 3);
}

#[test]
fn correctly_schedules_tasks_that_have_more_than_one_dependency() {
    let mut t = TestSuite::build();
    let f = t
        .add_nodes(vec!["A", "B", "C", "D", "E"])
        // All nodes depend on A, D depends on C and B as well
        .add_edges(vec![
            ("A", "B"),
            ("A", "C"),
            ("A", "D"),
            ("A", "E"),
            ("C", "D"),
            ("B", "D"),
        ])
        .run();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
        .unwrap();

    assert_ordering(&t.records.lock().unwrap().exec_records, "A", "B");
    assert_ordering(&t.records.lock().unwrap().exec_records, "A", "C");
    assert_ordering(&t.records.lock().unwrap().exec_records, "A", "D");
    assert_ordering(&t.records.lock().unwrap().exec_records, "A", "E");
    assert_ordering(&t.records.lock().unwrap().exec_records, "B", "D");
    assert_ordering(&t.records.lock().unwrap().exec_records, "C", "D");
}

fn should_schedule_high_priority_tasks_and_dependencies_before_lower_priority_tasks() {}

fn should_schedule_high_priority_tasks_and_dependencies_before_lower_priority_tasks_when_max_concurrency_is_greater_than_1(
) {
}

fn assert_ordering<T: AsRef<str>>(records: &[T], first: &str, second: &str) {
    assert!(first != second);
    let first_idx = records.iter().position(|s| s.as_ref() == first);
    let second_idx = records.iter().position(|s| s.as_ref() == second);
    assert!(first_idx.unwrap() < second_idx.unwrap())
}
