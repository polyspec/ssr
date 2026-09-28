use super::DevelopmentError;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct ProcessOptions {
    pub ready_timeout: Duration,
    pub drain_timeout: Duration,
    pub restart_limit: usize,
    pub event_capacity: usize,
    pub max_probe_bytes: usize,
}

impl ProcessOptions {
    pub fn validate(self) -> Result<(), DevelopmentError> {
        if self.ready_timeout.is_zero()
            || self.drain_timeout.is_zero()
            || self.event_capacity == 0
            || self.max_probe_bytes == 0
        {
            return Err(DevelopmentError(
                "process timeouts and capacities must be positive".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn validate_excluded(path: &Path) -> Result<(), DevelopmentError> {
    if !path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(DevelopmentError(
            "excluded path must be absolute without dot components".into(),
        ));
    }
    let mut existing = path;
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(_) => {
                let canonical = existing.canonicalize().map_err(|error| {
                    DevelopmentError(format!("excluded path inspection failed: {error}"))
                })?;
                if canonical != existing {
                    return Err(DevelopmentError(
                        "excluded path contains symbolic links".into(),
                    ));
                }
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing.parent().ok_or_else(|| {
                    DevelopmentError("excluded path has no existing parent".into())
                })?;
            }
            Err(error) => {
                return Err(DevelopmentError(format!(
                    "excluded path inspection failed: {error}"
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_excluded;
    use std::path::Path;

    #[test]
    fn excluded_paths_allow_missing_children_but_reject_relative_and_parent_paths() {
        assert!(
            validate_excluded(Path::new("/definitely-missing-ssr-process-test/generated")).is_ok()
        );
        assert!(validate_excluded(Path::new("relative/generated")).is_err());
        assert!(validate_excluded(Path::new("/app/../generated")).is_err());
    }
}
