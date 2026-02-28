//! Shared business logic for async_sleep and async_sleep_shared examples.

use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct SleepResult {
    pub completed: bool,
    pub task_ids: Vec<u8>,
    pub total_duration_ms: u64,
}

/// Runs 3 concurrent sleep tasks and returns a structured result.
#[cfg(feature = "tracing")]
pub async fn run_sleep_demo() -> SleepResult {
    tracing::info!("Starting sleep demo");
    let start = std::time::Instant::now();

    let t1 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        1u8
    });
    let t2 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        2u8
    });
    let t3 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        3u8
    });

    let (r1, r2, r3) = tokio::join!(t1, t2, t3);
    let task_ids = vec![r1.unwrap(), r2.unwrap(), r3.unwrap()];
    let total_duration_ms = start.elapsed().as_millis() as u64;

    tracing::info!("Sleep demo completed in {}ms", total_duration_ms);

    SleepResult {
        completed: true,
        task_ids,
        total_duration_ms,
    }
}
