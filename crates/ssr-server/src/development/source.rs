use super::DevelopmentError;
use notify::{Event, RecursiveMode};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod kqueue;

/// Receives every event and error that the file watch reports.
pub(super) type Handler = Box<dyn Fn(notify::Result<Event>) + Send + Sync + 'static>;

/// The file watch of the source paths. The platform watch implements it; tests replace it to
/// decide when events arrive.
pub(super) trait SourceWatch: Send {
    /// Registers `path`; every change after it returns produces an event.
    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> Result<(), DevelopmentError>;
}

/// The source watch of the platform: `kqueue(2)` on macOS, whose walk skips the excluded
/// paths, and `inotify(7)` through `notify` on Linux.
pub(super) fn platform(
    handler: Handler,
    excluded: Vec<PathBuf>,
) -> Result<Box<dyn SourceWatch>, DevelopmentError> {
    #[cfg(target_os = "macos")]
    {
        kqueue::start(handler, excluded)
    }
    #[cfg(not(target_os = "macos"))]
    {
        drop(excluded);
        notify::start(handler)
    }
}

#[cfg(not(target_os = "macos"))]
mod notify {
    use super::{DevelopmentError, Handler, SourceWatch};
    use notify::{RecursiveMode, Watcher};
    use std::path::Path;

    struct Notify(notify::RecommendedWatcher);

    impl SourceWatch for Notify {
        fn watch(&mut self, path: &Path, mode: RecursiveMode) -> Result<(), DevelopmentError> {
            self.0
                .watch(path, mode)
                .map_err(|error| DevelopmentError(format!("watch registration failed: {error}")))
        }
    }

    pub(super) fn start(handler: Handler) -> Result<Box<dyn SourceWatch>, DevelopmentError> {
        let watcher = notify::recommended_watcher(move |event| handler(event))
            .map_err(|e| DevelopmentError(format!("watch creation failed: {e}")))?;
        Ok(Box::new(Notify(watcher)))
    }
}

/// The file that `start` writes into its own watched directory; its event marks the baseline.
pub(super) const SENTINEL: &str = "baseline";

/// A private directory that `start` creates and watches after the source paths. The event of
/// its sentinel file marks the baseline: events that arrive before it belong to writes that the
/// first build reads, and every write after it produces an event that arrives after it.
pub(super) struct Baseline {
    directory: PathBuf,
    device: u64,
    inode: u64,
    closed: bool,
}

impl Baseline {
    pub fn new() -> Result<Self, DevelopmentError> {
        let root = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| DevelopmentError(format!("watch baseline root failed: {e}")))?;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random)
            .map_err(|e| DevelopmentError(format!("watch baseline name failed: {e}")))?;
        let name = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let directory = root.join(format!("ssr-watch-{name}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| DevelopmentError(format!("watch baseline directory failed: {e}")))?;
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|e| DevelopmentError(format!("watch baseline metadata failed: {e}")))?;
        Ok(Self {
            directory,
            device: metadata.dev(),
            inode: metadata.ino(),
            closed: false,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn sentinel(&self) -> PathBuf {
        self.directory.join(SENTINEL)
    }

    pub fn write(&self) -> Result<(), DevelopmentError> {
        std::fs::write(self.sentinel(), b"")
            .map_err(|e| DevelopmentError(format!("watch baseline write failed: {e}")))
    }

    pub fn close(mut self) -> Result<(), DevelopmentError> {
        self.closed = true;
        self.cleanup()
    }

    fn cleanup(&self) -> Result<(), DevelopmentError> {
        let metadata = std::fs::symlink_metadata(&self.directory)
            .map_err(|e| DevelopmentError(format!("watch baseline cleanup failed: {e}")))?;
        if !metadata.is_dir() || metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(DevelopmentError(
                "owned watch baseline directory differs".into(),
            ));
        }
        match std::fs::remove_file(self.sentinel()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(DevelopmentError(format!(
                    "watch baseline removal failed: {error}"
                )));
            }
        }
        std::fs::remove_dir(&self.directory)
            .map_err(|e| DevelopmentError(format!("watch baseline directory removal failed: {e}")))
    }
}

impl Drop for Baseline {
    fn drop(&mut self) {
        if !self.closed
            && let Err(error) = self.cleanup()
        {
            tracing::error!(%error, "watch baseline cleanup failed");
        }
    }
}
