use super::*;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

fn socket_command() -> (
    Arc<std::sync::Mutex<Option<PathBuf>>>,
    impl FnOnce(&Path) -> Command,
) {
    let path = Arc::new(std::sync::Mutex::new(None));
    let captured = path.clone();
    let executable = crate::development::test_programs::program("SSR_SOCKET_PROCESS");
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

async fn rejected_start(environment: &[(&str, &str)]) -> (DevelopmentError, PathBuf) {
    let (path, command) = socket_command();
    let (_cancel, canceled) = watch::channel(false);
    let result = Process::start(
        1,
        |socket| {
            let mut command = command(socket);
            command.envs(environment.iter().copied());
            command
        },
        b"probe",
        Duration::from_secs(5),
        1024,
        canceled,
    )
    .await;
    let socket = path.lock().unwrap().clone().unwrap();
    match result {
        Ok(process) => {
            let exit = process.wait(Duration::from_secs(2)).await;
            panic!("the declared socket was accepted; exit {:?}", exit.result);
        }
        Err(error) => (error, socket),
    }
}

#[tokio::test]
async fn the_owned_socket_is_absolute_in_its_own_private_directory() {
    let (path, command) = socket_command();
    let (_cancel, canceled) = watch::channel(false);
    let process = Process::start(1, command, b"probe", Duration::from_secs(5), 1024, canceled)
        .await
        .unwrap();
    let socket = path.lock().unwrap().clone().unwrap();
    let directory = socket.parent().unwrap().to_path_buf();
    let absolute = socket.is_absolute() && socket.canonicalize().unwrap() == socket;
    let mode = std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777;
    let entries = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    process.wait(Duration::from_secs(2)).await.result.unwrap();
    assert!(
        absolute,
        "socket path is not absolute and canonical: {}",
        socket.display()
    );
    assert_eq!(mode, 0o700, "socket directory mode {mode:o}");
    assert_eq!(entries, vec![socket.clone()]);
    assert!(!directory.exists());
}

#[tokio::test]
async fn a_network_readiness_address_is_rejected() {
    let (error, socket) = rejected_start(&[("SOCKET_DECLARE", "http://127.0.0.1:9/")]).await;
    assert!(
        error.to_string().contains("differs from the owned path"),
        "{error}"
    );
    assert!(!socket.parent().unwrap().exists());
}

#[tokio::test]
async fn a_relative_declared_path_is_rejected() {
    let (error, socket) = rejected_start(&[("SOCKET_DECLARE", "render")]).await;
    assert!(
        error.to_string().contains("differs from the owned path"),
        "{error}"
    );
    assert!(!socket.parent().unwrap().exists());
}

#[tokio::test]
async fn a_declared_path_that_is_not_a_socket_is_rejected() {
    let (error, socket) = rejected_start(&[("SOCKET_KIND", "file")]).await;
    assert!(
        error
            .to_string()
            .contains("render socket is not a Unix socket"),
        "{error}"
    );
    assert!(!socket.parent().unwrap().exists());
}

#[tokio::test]
async fn a_socket_directory_open_to_other_users_is_rejected() {
    let (error, socket) = rejected_start(&[("SOCKET_KIND", "shared")]).await;
    assert!(
        error
            .to_string()
            .contains("render socket directory is open to other users: mode 755"),
        "{error}"
    );
    assert!(!socket.parent().unwrap().exists());
}

#[tokio::test]
async fn a_socket_directory_with_another_entry_is_rejected_and_kept() {
    let (error, socket) = rejected_start(&[("SOCKET_KIND", "extra")]).await;
    let directory = socket.parent().unwrap().to_path_buf();
    let extra = directory.join("extra");
    let kept = extra.is_file() && !socket.exists();
    if kept {
        std::fs::remove_file(&extra).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }
    assert!(kept, "the supervisor removed a file it did not create");
    assert!(
        error
            .to_string()
            .contains("render socket requires its own private directory"),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("socket directory removal failed"),
        "{error}"
    );
}

#[test]
fn a_socket_path_over_the_address_limit_is_rejected_before_creation() {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap();
    let name = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("{name}-{}", "a".repeat(100)));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let result = super::super::socket::Socket::create(&root);
    let entries = std::fs::read_dir(&root).unwrap().count();
    let error = result.err().map(|error| error.to_string());
    std::fs::remove_dir(&root).unwrap();
    assert!(
        error.as_deref().is_some_and(
            |error| error.contains("render socket path exceeds the Unix socket address length")
        ),
        "{error:?}"
    );
    assert_eq!(
        entries, 0,
        "a socket directory was created for an unusable path"
    );
}
