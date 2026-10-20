use super::DevelopmentError;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(super) struct Socket {
    directory: PathBuf,
    path: PathBuf,
    device: u64,
    inode: u64,
    closed: bool,
}

impl Socket {
    pub fn new() -> Result<Self, DevelopmentError> {
        let root = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| DevelopmentError::with_source(format!("socket root failed: {e}"), e))?;
        Self::create(&root)
    }

    pub fn create(root: &Path) -> Result<Self, DevelopmentError> {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random)
            .map_err(|e| DevelopmentError::with_source(format!("socket name failed: {e}"), e))?;
        let name = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let directory = root.join(format!("ssr-{name}"));
        let path = directory.join("render");
        std::os::unix::net::SocketAddr::from_pathname(&path).map_err(|e| {
            DevelopmentError::with_source(format!(
                "render socket path exceeds the Unix socket address length: {}: {e}",
                path.display()
            ), e)
        })?;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| DevelopmentError::with_source(format!("socket directory creation failed: {e}"), e))?;
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|e| DevelopmentError::with_source(format!("socket directory metadata failed: {e}"), e))?;
        Ok(Self {
            path,
            directory,
            device: metadata.dev(),
            inode: metadata.ino(),
            closed: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn validate(&self, socket: &Path) -> Result<(), DevelopmentError> {
        if socket != self.path() {
            return Err(DevelopmentError::new("render socket differs from the owned path",
            ));
        }
        if !socket.is_absolute()
            || socket
                .canonicalize()
                .map_err(|e| DevelopmentError::with_source(format!("render socket path failed: {e}"), e))?
                != socket
        {
            return Err(DevelopmentError::new("render socket path must be absolute and canonical",
            ));
        }
        let metadata = std::fs::symlink_metadata(socket)
            .map_err(|e| DevelopmentError::with_source(format!("render socket metadata failed: {e}"), e))?;
        let directory = socket
            .parent()
            .ok_or_else(|| DevelopmentError::new("render socket parent is missing"))?;
        let mode = std::fs::metadata(directory)
            .map_err(|e| DevelopmentError::with_source(format!("render socket directory failed: {e}"), e))?
            .permissions()
            .mode();
        if !metadata.file_type().is_socket() {
            return Err(DevelopmentError::new("render socket is not a Unix socket",
            ));
        }
        if mode & 0o077 != 0 {
            return Err(DevelopmentError::new(format!(
                "render socket directory is open to other users: mode {:o}",
                mode & 0o777
            )));
        }
        let entries = std::fs::read_dir(directory)
            .map_err(|e| DevelopmentError::with_source(format!("render socket directory read failed: {e}"), e))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DevelopmentError::with_source(format!("render socket directory entry failed: {e}"), e))?;
        if entries != vec![socket.to_path_buf()] {
            return Err(DevelopmentError::new("render socket requires its own private directory",
            ));
        }
        Ok(())
    }

    pub fn close(mut self) -> Result<(), DevelopmentError> {
        let result = self.cleanup();
        self.closed = true;
        result
    }

    fn cleanup(&self) -> Result<(), DevelopmentError> {
        let metadata = std::fs::symlink_metadata(&self.directory)
            .map_err(|e| DevelopmentError::with_source(format!("socket cleanup metadata failed: {e}"), e))?;
        if !metadata.is_dir() || metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(DevelopmentError::new("owned socket directory differs"));
        }
        match std::fs::symlink_metadata(&self.path) {
            Ok(_) => std::fs::remove_file(&self.path)
                .map_err(|e| DevelopmentError::with_source(format!("socket removal failed: {e}"), e))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(DevelopmentError::with_source(format!(
                    "socket cleanup path failed: {error}"
                ), error));
            }
        }
        std::fs::remove_dir(&self.directory)
            .map_err(|e| DevelopmentError::with_source(format!("socket directory removal failed: {e}"), e))
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if !self.closed
            && let Err(error) = self.cleanup()
        {
            eprintln!("{error}");
        }
    }
}
