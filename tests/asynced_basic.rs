#![feature(type_alias_impl_trait)]
use async_trait::async_trait;

use std::{sync::Mutex, time::Duration};

use graph_executor::{AsyncExecutable, AsyncGraphExecutor};
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

#[async_trait]
impl AsyncExecutable for HunmanBehavior {
    async fn exec(&mut self) -> anyhow::Result<()> {
        println!("exec {}", self.id);
        tokio::time::sleep(Duration::from_millis(10000)).await;
        // std::thread::sleep();
        RECORDS.lock().unwrap().push(self.id);
        Ok(())
    }
}

#[test]
fn basic() {
    type TestGraphRunner = AsyncGraphExecutor<&'static str, HunmanBehavior>;
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
    // futures::executor::block_on(runner.exec());
    tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(runner.exec());
    
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
