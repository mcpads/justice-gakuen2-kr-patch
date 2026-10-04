//! Join every scoped component before propagating a failure. Dropping a handle
//! leaves joining to `thread::scope`, which propagates an unobserved panic.
use anyhow::{Result, anyhow};

pub(super) fn finish_component_build<T>(
    handle: std::thread::ScopedJoinHandle<'_, Result<T>>,
    stage: &str,
) -> Result<T> {
    handle
        .join()
        .map_err(|_| anyhow!("{stage} build worker panicked"))?
}

macro_rules! join_component_builds {
    ($($worker:ident => $stage:expr),+ $(,)?) => {{
        // Do not use `?` until every handle has been explicitly joined.
        $(let $worker = $crate::dialogue_audit::component_workers::finish_component_build(
            $worker, $stage,
        );)+
        Ok::<_, anyhow::Error>(($($worker?,)+))
    }};
}
pub(super) use join_component_builds;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn earlier_error_and_later_panic_return_an_error_after_every_worker_finishes() {
        let finished = AtomicBool::new(false);
        let result = std::panic::catch_unwind(|| {
            std::thread::scope(|scope| {
                let first = scope.spawn(|| -> Result<()> { anyhow::bail!("invalid input") });
                let second = scope.spawn(|| -> Result<()> { panic!("injected worker panic") });
                let third = scope.spawn(|| {
                    finished.store(true, Ordering::SeqCst);
                    Ok(7)
                });
                join_component_builds!(first => "first", second => "second", third => "third")
            })
        });
        let failure = result
            .expect("worker panic must become a build error")
            .unwrap_err();
        assert_eq!(failure.to_string(), "invalid input");
        assert!(finished.load(Ordering::SeqCst));
    }

    #[test]
    fn multiple_panics_are_joined_and_reported_in_declared_order() {
        let result = std::panic::catch_unwind(|| {
            std::thread::scope(|scope| {
                let first = scope.spawn(|| -> Result<()> { panic!("first panic") });
                let second = scope.spawn(|| -> Result<()> { panic!("second panic") });
                join_component_builds!(first => "first", second => "second")
            })
        });
        let failure = result
            .expect("all worker panics must be consumed")
            .unwrap_err();
        assert_eq!(failure.to_string(), "first build worker panicked");
    }

    #[test]
    fn successful_components_retain_their_types_and_order() {
        let result = std::thread::scope(|scope| {
            let label = scope.spawn(|| Ok(String::from("label")));
            let count = scope.spawn(|| Ok(12_u16));
            join_component_builds!(label => "label", count => "count")
        })
        .unwrap();
        assert_eq!(result, (String::from("label"), 12_u16));
    }
}
