// The engine lock and the renderer state are process-wide, and a panic while initializing the snapshot poisons the lock for
// the rest of the process. The poison case therefore runs in its own test binary, so that it cannot affect the test of
// process.rs, which runs in a process of its own.
use polyspec_ssr_core::process::{build, render};

#[test]
fn snapshot_panic_poison_is_not_hidden() {
    assert!(
        std::panic::catch_unwind(|| render(|| -> Result<(), ()> {
            panic!("snapshot initialization panicked");
        }))
        .is_err()
    );
    assert_eq!(build(|| ()), Err("process engine lock is poisoned"));
    assert_eq!(
        render(|| Ok::<_, ()>(())),
        Err("process engine lock is poisoned")
    );
}
