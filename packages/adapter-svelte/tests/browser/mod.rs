use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

/// The chromium build of the package installation, which the nextest setup script
/// `install-packages` names (`tools/install_packages.py`): the same browser on every machine.
pub fn browser() -> String {
    let path = std::env::var("SSR_BROWSER").unwrap_or_else(|_| {
        panic!(
            "SSR_BROWSER is not set; the nextest setup script install-packages sets it, so run \
             the tests with cargo nextest"
        )
    });
    assert!(
        Path::new(&path).is_absolute() && Path::new(&path).is_file(),
        "SSR_BROWSER must name the chromium executable: {path}"
    );
    path
}

/// Runs a browser script until its process exits and returns its standard output.
///
/// Each output line is printed with the elapsed time when it arrives. The script bounds
/// each page step and gives the browser launch and close no limit; the nextest browser
/// override bounds the whole case.
pub fn run(script: &Path, base: &str, arguments: &[&str]) -> String {
    let browser = browser();
    assert!(
        script.is_file(),
        "browser test script missing: {}",
        script.display()
    );
    let started = Instant::now();
    println!("RUN browser {}", script.display());
    let mut child = Command::new("node")
        .arg(script)
        .arg(base)
        .arg(&browser)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let errors = thread::spawn(move || {
        let mut text = String::new();
        for line in BufReader::new(stderr).lines() {
            let line = line.unwrap();
            eprintln!("{:.3}s {line}", started.elapsed().as_secs_f64());
            text.push_str(&line);
            text.push('\n');
        }
        text
    });
    let mut output = String::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        println!("{:.3}s {line}", started.elapsed().as_secs_f64());
        output.push_str(&line);
        output.push('\n');
    }
    let status = child.wait().unwrap();
    let errors = errors.join().unwrap();
    println!(
        "{:.3}s browser exited: {status}",
        started.elapsed().as_secs_f64()
    );
    assert!(
        status.success(),
        "browser failed: stdout: {output}; stderr: {errors}"
    );
    output
}
