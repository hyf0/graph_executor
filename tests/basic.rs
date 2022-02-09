use std::{sync::Mutex, time::Duration};

use graph_executor::{Executable, GraphExecutor};
use once_cell::sync::Lazy;

#[derive(Debug, Default)]
struct HunmanBehavior {
    pub id: &'static str,
}

impl HunmanBehavior {
    pub fn new(id: &'static str) -> Self {
        Self { id }
    }
}

static RECORDS: Lazy<Mutex<Vec<&'static str>>> = Lazy::new(|| Mutex::new(vec![]));

impl Executable for HunmanBehavior {
    fn exec(&mut self) {
        std::thread::sleep(Duration::from_millis(100));
        RECORDS.lock().unwrap().push(self.id);
        println!("exec {}", self.id)
    }
}

#[test]
fn basic() {
    type TestGraphRunner = GraphExecutor<&'static str, HunmanBehavior>;
    let nodes = [
        ("putOnShirt", HunmanBehavior::new("putOnShirt")),
        ("putOnShorts", HunmanBehavior::new("putOnShorts")),
        ("putOnJacket", HunmanBehavior::new("putOnJacket")),
        ("putOnShoes", HunmanBehavior::new("putOnShoes")),
        ("tieShoes", HunmanBehavior::new("tieShoes")),
    ]
    .into_iter()
    .collect();
    //       putOnShorts         putOnShirt
    //       /         \          /
    //  putOnShoes      putOnJacket
    //           \
    //            tieShoes
    let mut runner = TestGraphRunner::new(
        nodes,
        vec![
            ("putOnShirt", "putOnJacket"),
            ("putOnShoes", "tieShoes"),
            ("putOnShorts", "putOnJacket"),
            ("putOnShorts", "putOnShoes"),
        ],
    );
    runner.exec();

    assert_ordering(&RECORDS.lock().unwrap(), "putOnShoes", "tieShoes");
    assert_ordering(&RECORDS.lock().unwrap(), "putOnShirt", "putOnJacket");
    assert_ordering(&RECORDS.lock().unwrap(), "putOnShorts", "putOnJacket");
    assert_ordering(&RECORDS.lock().unwrap(), "putOnShorts", "putOnShoes");
    // runner.todo_queue.lock().unwrap().push("a", 0);
    // println!("running");
    // println!("running {:#?}", runner);
}

fn assert_ordering<T: AsRef<str>>(records: &[T], first: &str, second: &str) {
    assert!(first != second);
    let first_idx = records.iter().position(|s| s.as_ref() == first);
    let second_idx = records.iter().position(|s| s.as_ref() == second);
    assert!(first_idx.unwrap() < second_idx.unwrap())
}
