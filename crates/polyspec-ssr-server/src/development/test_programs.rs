use std::path::PathBuf;

/// The program that the nextest setup script `build-programs` built for this run and named in
/// `variable` (`tools/build_programs.py`).
pub(crate) fn program(variable: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| {
        panic!(
            "{variable} is not set; it names the program that the nextest setup script \
             build-programs builds, so run the tests with cargo nextest"
        )
    }));
    assert!(
        path.is_absolute() && path.is_file(),
        "{variable} must name the built program, an absolute file: {}",
        path.display()
    );
    path
}
