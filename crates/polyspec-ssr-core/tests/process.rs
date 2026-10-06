use polyspec_ssr_core::process::{build, render};
use std::sync::mpsc::sync_channel;
use std::time::Duration;

#[test]
fn process_serializes_compilers_and_snapshot_selection() {
    let (entered_tx, entered_rx) = sync_channel(1);
    let (finish_tx, finish_rx) = sync_channel(1);
    let compiler = std::thread::spawn(move || {
        build(|| {
            entered_tx.send(()).unwrap();
            finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            17
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(build(|| 23), Ok(23));
    assert_eq!(
        render(|| Ok::<_, ()>(())),
        Err("renderer initialization cannot run while another engine operation is active")
    );
    finish_tx.send(()).unwrap();
    assert_eq!(compiler.join().unwrap(), Ok(17));

    assert_eq!(
        render(|| Err::<(), _>("initialization failed")),
        Ok(Err("initialization failed"))
    );
    assert_eq!(build(|| 31), Ok(31));
    assert_eq!(
        render(|| {
            assert_eq!(
                build(|| ()),
                Err("Svelte compilation cannot run while renderer initialization is active")
            );
            assert_eq!(
                render(|| Ok::<_, ()>(())),
                Err("renderer initialization cannot run while another engine operation is active")
            );
            Ok::<_, ()>(41)
        }),
        Ok(Ok(41))
    );
    assert_eq!(
        build(|| ()),
        Err("Svelte compilation cannot run after renderer initialization")
    );
    assert_eq!(
        render(|| Ok::<_, ()>(())),
        Err("renderer process already initialized a snapshot")
    );
}

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
