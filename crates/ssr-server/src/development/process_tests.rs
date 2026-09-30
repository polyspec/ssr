use super::*;
use std::os::unix::fs::DirBuilderExt;

fn socket_command() -> (
    Arc<std::sync::Mutex<Option<PathBuf>>>,
    impl FnOnce(&Path) -> Command,
) {
    let path = Arc::new(std::sync::Mutex::new(None));
    let captured = path.clone();
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/socket_process");
    assert!(
        executable.is_file(),
        "build the maintained socket_process example"
    );
    (path, move |socket: &Path| {
        *captured.lock().unwrap() = Some(socket.to_path_buf());
        let mut command = Command::new(executable);
        command.arg(socket);
        command
    })
}

#[tokio::test]
async fn private_socket_requests_do_not_resolve_a_service_name() {
    let (_path, command) = socket_command();
    let (_cancel, canceled) = watch::channel(false);
    let result = Process::start(1, command, b"probe", Duration::from_secs(5), 1024, canceled).await;
    let output = match result {
        Ok(process) => {
            let generation = process.generation.clone();
            let waiter = tokio::spawn(process.wait(Duration::from_secs(2)));
            let response = generation
                .request(
                    Request::builder()
                        .method("POST")
                        .uri("/_render?case=socket")
                        .header(http::header::HOST, "app.example.test")
                        .body(b"probe".to_vec())
                        .unwrap(),
                )
                .await;
            let output = match response {
                Ok(response) => Ok(response.into_body().collect().await.unwrap().to_bytes()),
                Err(error) => Err(error),
            };
            drop(generation);
            let exit = waiter.await.unwrap();
            eprintln!(
                "socket process exit expected={} result={:?}",
                exit.expected, exit.result
            );
            exit.result.unwrap();
            output
        }
        Err(error) => Err(error),
    };
    assert_eq!(output.unwrap(), "<p>ready</p>");
}

#[tokio::test]
async fn completed_process_removes_its_declared_socket() {
    let (path, command) = socket_command();
    let (_cancel, canceled) = watch::channel(false);
    let process = Process::start(1, command, b"probe", Duration::from_secs(5), 1024, canceled)
        .await
        .unwrap();
    process.wait(Duration::from_secs(2)).await.result.unwrap();
    let socket = path.lock().unwrap().clone().unwrap();
    let remains = socket.exists() || socket.parent().unwrap().exists();
    assert!(!remains, "completed renderer leaves its declared socket");
}

#[tokio::test]
async fn a_different_declared_socket_is_rejected_and_preserved() {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap();
    let name = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let directory = std::env::temp_dir().canonicalize().unwrap().join(name);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let foreign = directory.join("other");
    let listener = tokio::net::UnixListener::bind(&foreign).unwrap();
    let (path, command) = socket_command();
    let (_cancel, canceled) = watch::channel(false);
    let result = Process::start(
        1,
        |socket| {
            let mut command = command(socket);
            command.env("SOCKET_DECLARE", &foreign);
            command
        },
        b"probe",
        Duration::from_secs(5),
        1024,
        canceled,
    )
    .await;
    let preserved = foreign.exists();
    drop(listener);
    std::fs::remove_file(&foreign).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    let error = result.err().unwrap();
    assert!(
        error.to_string().contains("differs from the owned path"),
        "{error}"
    );
    assert!(preserved, "another process socket was removed");
    let owned = path.lock().unwrap().clone().unwrap();
    assert!(!owned.exists());
    assert!(!owned.parent().unwrap().exists());
}
