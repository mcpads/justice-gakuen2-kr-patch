use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};

pub(crate) fn map_ordered_parallel<T, U, F>(items: &[T], task: F) -> Result<Vec<U>>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> Result<U> + Sync,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(items.len());
    let next_index = AtomicUsize::new(0);
    let results = (0..items.len())
        .map(|_| Mutex::new(None))
        .collect::<Vec<_>>();

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let index = next_index.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    *results[index]
                        .lock()
                        .expect("ordered parallel result mutex was poisoned") = Some(task(item));
                }
            });
        }
    });

    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result
                .into_inner()
                .expect("ordered parallel result mutex was poisoned")
                .with_context(|| format!("ordered parallel worker {index} produced no result"))?
        })
        .collect()
}
