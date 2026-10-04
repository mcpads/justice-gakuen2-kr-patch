use super::ordered_parallel::map_ordered_parallel;

#[test]
fn parallel_results_keep_source_order() {
    let results = map_ordered_parallel(&[4_u64, 1, 3, 2], |value| {
        std::thread::sleep(std::time::Duration::from_millis(*value));
        Ok(value * 10)
    })
    .unwrap();

    assert_eq!(results, [40, 10, 30, 20]);
}

#[test]
fn parallel_worker_errors_are_returned_in_source_order() {
    let error = map_ordered_parallel(&[0, 1, 2], |value| {
        if *value == 1 {
            anyhow::bail!("rejected item")
        }
        Ok(*value)
    })
    .unwrap_err();

    assert!(error.to_string().contains("rejected item"));
}
