use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use graph_executor::{AsyncExecutable, AsyncGraphExecutor};

#[derive(Debug)]
struct HumanBehavior {
    start_time: Instant,
    id: &'static str,
}

#[async_trait]
impl AsyncExecutable for HumanBehavior {
    async fn exec(&mut self) -> anyhow::Result<()> {
        let now = Instant::now();
        let diff = now.duration_since(self.start_time);
        let secs = diff.as_secs();
        for _ in 0..secs {
            print!("____second____");
        }
        println!("I'm doing {}", self.id);
        tokio::time::sleep(Duration::from_millis(1000)).await;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let now = Instant::now();
    let putOnShirt = HumanBehavior {
        id: "putOnShirt",
        start_time: now,
    };
    let putOnShorts = HumanBehavior {
        id: "putOnShorts",
        start_time: now,
    };
    let putOnJacket = HumanBehavior {
        id: "putOnJacket",
        start_time: now,
    };
    let putOnShoes = HumanBehavior {
        id: "putOnShoes",
        start_time: now,
    };
    let tieShoes = HumanBehavior {
        id: "tieShoes",
        start_time: now,
    };
    let nodes = vec![
        ("putOnShirt", putOnShirt),
        ("putOnShorts", putOnShorts),
        ("putOnJacket", putOnJacket),
        ("putOnShoes", putOnShoes),
        ("tieShoes", tieShoes),
    ]
    .into_iter()
    .collect::<HashMap<_, _>>();
    //       putOnShorts         putOnShirt
    //       /         \          /
    //  putOnShoes      putOnJacket
    //           \
    //            tieShoes
    let edges = vec![
        ("putOnShirt", "putOnJacket"),
        ("putOnShoes", "tieShoes"),
        ("putOnShorts", "putOnJacket"),
        ("putOnShorts", "putOnShoes"),
    ];
    AsyncGraphExecutor::new(nodes, edges).exec().await.unwrap();
    // output =>
    // I'm doing putOnShirt
    // I'm doing putOnShorts
    // ____second____I'm doing putOnJacket
    // ____second____I'm doing putOnShoes
    // ____second________second____I'm doing tieShoes
}
