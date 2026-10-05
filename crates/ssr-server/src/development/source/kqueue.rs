//! The source watch of macOS. It registers every watched directory and regular file with
//! `kqueue(2)`, which reports each change of a registered vnode synchronously and without a
//! daemon in between. The walk honours the excluded paths. A change of a directory rescans it:
//! new entries are registered before their creation is reported, so a write to a new file is
//! either read by the build that the creation starts or reported as a change of the file.

use super::super::DevelopmentError;
use super::{Handler, SourceWatch};
use kqueue::{EventData, EventFilter, FilterFlag, Ident, Vnode};
use notify::event::{
    CreateKind, DataChange, EventKind, MetadataKind, ModifyKind, RemoveKind, RenameMode,
};
use notify::{Event, RecursiveMode};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;

#[cfg(test)]
#[path = "kqueue_tests.rs"]
mod tests;

/// A watched path and, for a directory, whether its subdirectories are watched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entry {
    Directory { recursive: bool },
    File,
}

/// A change that the kernel reported for a registered path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    Write,
    Size,
    Attributes,
    Link,
    Delete,
    Rename,
    Revoke,
}

/// The kernel registration of paths. After `register` returns, every change of a registered
/// path produces a change event.
pub(super) trait Kernel {
    /// Registers the paths and returns those that still existed; a path that disappeared
    /// before its registration is reported by the rescan of its directory.
    fn register(&mut self, paths: &[PathBuf]) -> Result<Vec<PathBuf>, DevelopmentError>;
    fn unregister(&mut self, path: &Path) -> Result<(), DevelopmentError>;
}

/// The watched paths of the source watch and the rules that keep them in step with the
/// directories.
pub(super) struct Tree {
    excluded: Vec<PathBuf>,
    watched: BTreeMap<PathBuf, Entry>,
}

impl Tree {
    pub fn new(excluded: Vec<PathBuf>) -> Self {
        Self {
            excluded,
            watched: BTreeMap::new(),
        }
    }

    pub fn add(
        &mut self,
        kernel: &mut impl Kernel,
        path: &Path,
        mode: RecursiveMode,
    ) -> Result<(), DevelopmentError> {
        let entry = if path.is_dir() {
            Entry::Directory {
                recursive: mode == RecursiveMode::Recursive,
            }
        } else {
            Entry::File
        };
        self.register(kernel, vec![(path.to_path_buf(), entry)])?;
        Ok(())
    }

    /// Applies a change of `path` and returns the events to report.
    pub fn change(
        &mut self,
        kernel: &mut impl Kernel,
        path: &Path,
        change: Change,
    ) -> Result<Vec<Event>, DevelopmentError> {
        let Some(entry) = self.watched.get(path).copied() else {
            tracing::debug!(path = %path.display(), ?change, "change of a path that is no longer watched");
            return Ok(Vec::new());
        };
        let event = |kind| vec![Event::new(kind).add_path(path.to_path_buf())];
        Ok(match (entry, change) {
            (_, Change::Delete | Change::Revoke) => {
                self.remove(kernel, path)?;
                event(EventKind::Remove(RemoveKind::Any))
            }
            (_, Change::Rename) => {
                self.remove(kernel, path)?;
                event(EventKind::Modify(ModifyKind::Name(RenameMode::Any)))
            }
            (Entry::Directory { recursive }, Change::Write | Change::Size | Change::Link) => {
                self.rescan(kernel, path, recursive)?
            }
            (Entry::File, Change::Write) => {
                event(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            }
            (Entry::File, Change::Size) => {
                event(EventKind::Modify(ModifyKind::Data(DataChange::Size)))
            }
            (Entry::File, Change::Link) | (_, Change::Attributes) => {
                event(EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)))
            }
        })
    }

    fn rescan(
        &mut self,
        kernel: &mut impl Kernel,
        directory: &Path,
        recursive: bool,
    ) -> Result<Vec<Event>, DevelopmentError> {
        let mut events = Vec::new();
        let children = self
            .watched
            .keys()
            .filter(|path| path.parent() == Some(directory))
            .cloned()
            .collect::<Vec<_>>();
        for child in children {
            match std::fs::symlink_metadata(&child) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    self.remove(kernel, &child)?;
                    events.push(Event::new(EventKind::Remove(RemoveKind::Any)).add_path(child));
                }
                Err(error) => {
                    return Err(DevelopmentError(format!(
                        "source watch of {} failed: {error}",
                        child.display()
                    )));
                }
            }
        }
        let entries = self.unwatched_entries(directory, recursive)?;
        for (path, entry) in self.register(kernel, entries)? {
            let kind = match entry {
                Entry::Directory { .. } => CreateKind::Folder,
                Entry::File => CreateKind::File,
            };
            events.push(Event::new(EventKind::Create(kind)).add_path(path));
        }
        Ok(events)
    }

    /// Registers the paths level by level. The entries of a directory are read only after the
    /// directory is registered, so an entry created later produces a change of the directory.
    fn register(
        &mut self,
        kernel: &mut impl Kernel,
        start: Vec<(PathBuf, Entry)>,
    ) -> Result<Vec<(PathBuf, Entry)>, DevelopmentError> {
        let mut added = Vec::new();
        let mut pending = start;
        while !pending.is_empty() {
            let paths = pending
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            let registered = kernel
                .register(&paths)?
                .into_iter()
                .collect::<BTreeSet<_>>();
            let mut next = Vec::new();
            for (path, entry) in pending {
                if !registered.contains(&path) {
                    continue;
                }
                self.watched.insert(path.clone(), entry);
                if let Entry::Directory { recursive } = entry {
                    next.extend(self.unwatched_entries(&path, recursive)?);
                }
                added.push((path, entry));
            }
            pending = next;
        }
        Ok(added)
    }

    fn unwatched_entries(
        &self,
        directory: &Path,
        recursive: bool,
    ) -> Result<Vec<(PathBuf, Entry)>, DevelopmentError> {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(DevelopmentError(format!(
                    "source watch of {} failed: {error}",
                    directory.display()
                )));
            }
        };
        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                DevelopmentError(format!(
                    "source watch of {} failed: {e}",
                    directory.display()
                ))
            })?;
            let path = entry.path();
            if self.watched.contains_key(&path) || self.is_excluded(&path) {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(DevelopmentError(format!(
                        "source watch of {} failed: {error}",
                        path.display()
                    )));
                }
            };
            if file_type.is_dir() {
                if recursive {
                    found.push((path, Entry::Directory { recursive }));
                }
            } else if file_type.is_file() {
                found.push((path, Entry::File));
            } else {
                tracing::warn!(path = %path.display(), "source watch skips an entry that is neither a regular file nor a directory");
            }
        }
        Ok(found)
    }

    fn remove(&mut self, kernel: &mut impl Kernel, path: &Path) -> Result<(), DevelopmentError> {
        let paths = self
            .watched
            .range(path.to_path_buf()..)
            .map(|(watched, _)| watched)
            .take_while(|watched| watched.starts_with(path))
            .cloned()
            .collect::<Vec<_>>();
        for watched in paths {
            self.watched.remove(&watched);
            kernel.unregister(&watched)?;
        }
        Ok(())
    }

    fn is_excluded(&self, path: &Path) -> bool {
        self.excluded
            .iter()
            .any(|excluded| path.starts_with(excluded))
    }
}

/// The paths that a watch of `path` registers, counted before any of them is opened.
pub(super) struct Walk {
    pub paths: u64,
    pub directories: Vec<(PathBuf, u64)>,
}

impl Walk {
    pub fn count(
        path: &Path,
        mode: RecursiveMode,
        excluded: &[PathBuf],
    ) -> Result<Self, DevelopmentError> {
        let mut walk = Self {
            paths: 1,
            directories: Vec::new(),
        };
        if !path.is_dir() {
            return Ok(walk);
        }
        let mut pending = vec![path.to_path_buf()];
        while let Some(directory) = pending.pop() {
            let mut count = 0;
            let entries = std::fs::read_dir(&directory).map_err(|e| {
                DevelopmentError(format!(
                    "source walk of {} failed: {e}",
                    directory.display()
                ))
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| {
                    DevelopmentError(format!(
                        "source walk of {} failed: {e}",
                        directory.display()
                    ))
                })?;
                let path = entry.path();
                if excluded.iter().any(|excluded| path.starts_with(excluded)) {
                    continue;
                }
                let file_type = entry.file_type().map_err(|e| {
                    DevelopmentError(format!("source walk of {} failed: {e}", path.display()))
                })?;
                if file_type.is_dir() && mode == RecursiveMode::Recursive {
                    count += 1;
                    pending.push(path);
                } else if file_type.is_file() {
                    count += 1;
                }
            }
            walk.paths += count;
            walk.directories.push((directory, count));
        }
        walk.directories
            .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        walk.directories.truncate(5);
        Ok(walk)
    }

    fn largest(&self) -> String {
        self.directories
            .iter()
            .map(|(path, count)| format!("{} ({count})", path.display()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Decides the soft descriptor limit for `needed` more descriptors with `open` already open.
/// The limit is kept when they fit, otherwise raised by `needed` or to the hard limit; `Err`
/// names the hard limit when they do not fit under it.
pub(super) fn descriptor_limit(
    needed: u64,
    open: u64,
    soft: u64,
    hard: Option<u64>,
) -> Result<Option<u64>, u64> {
    let required = open.saturating_add(needed);
    if required <= soft {
        return Ok(None);
    }
    let cap = hard.unwrap_or(u64::MAX);
    let raised = soft.saturating_add(needed);
    if raised <= cap {
        Ok(Some(raised))
    } else if required <= cap {
        Ok(Some(cap))
    } else {
        Err(cap)
    }
}

fn ensure_descriptors(path: &Path, walk: &Walk) -> Result<(), DevelopmentError> {
    use rustix::process::{Resource, getrlimit, setrlimit};
    let open = std::fs::read_dir("/dev/fd")
        .map_err(|e| DevelopmentError(format!("open descriptor count failed: {e}")))?
        .count() as u64;
    let limit = getrlimit(Resource::Nofile);
    let soft = limit.current.unwrap_or(u64::MAX);
    let describe = |limit: String| {
        format!(
            "source watch of {} needs {} file descriptors with {open} open, above the descriptor limit {limit}; largest directories: {}",
            path.display(),
            walk.paths,
            walk.largest()
        )
    };
    match descriptor_limit(walk.paths, open, soft, limit.maximum) {
        Ok(None) => Ok(()),
        Ok(Some(raised)) => {
            setrlimit(
                Resource::Nofile,
                rustix::process::Rlimit {
                    current: Some(raised),
                    maximum: limit.maximum,
                },
            )
            .map_err(|e| {
                DevelopmentError(describe(format!(
                    "{soft}; raising it to {raised} failed: {e}"
                )))
            })?;
            tracing::info!(
                path = %path.display(),
                needed = walk.paths,
                open,
                from = soft,
                to = raised,
                "source watch raised the descriptor limit"
            );
            Ok(())
        }
        Err(hard) => Err(DevelopmentError(describe(format!(
            "{soft} (hard limit {hard})"
        )))),
    }
}

const VNODE: FilterFlag = FilterFlag::NOTE_DELETE
    .union(FilterFlag::NOTE_WRITE)
    .union(FilterFlag::NOTE_EXTEND)
    .union(FilterFlag::NOTE_ATTRIB)
    .union(FilterFlag::NOTE_LINK)
    .union(FilterFlag::NOTE_RENAME)
    .union(FilterFlag::NOTE_REVOKE);

struct Queue(kqueue::Watcher);

impl Kernel for Queue {
    fn register(&mut self, paths: &[PathBuf]) -> Result<Vec<PathBuf>, DevelopmentError> {
        let mut registered = Vec::with_capacity(paths.len());
        for path in paths {
            match self.0.add_filename(path, EventFilter::EVFILT_VNODE, VNODE) {
                Ok(()) => registered.push(path.clone()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    tracing::debug!(path = %path.display(), "source path disappeared before its registration");
                }
                Err(error) => {
                    return Err(DevelopmentError(format!(
                        "source watch of {} failed: {error}",
                        path.display()
                    )));
                }
            }
        }
        self.0
            .watch()
            .map_err(|e| DevelopmentError(format!("source watch registration failed: {e}")))?;
        Ok(registered)
    }

    fn unregister(&mut self, path: &Path) -> Result<(), DevelopmentError> {
        self.0
            .remove_filename(path, EventFilter::EVFILT_VNODE)
            .map_err(|e| {
                DevelopmentError(format!("source unwatch of {} failed: {e}", path.display()))
            })
    }
}

enum Command {
    Add(
        PathBuf,
        RecursiveMode,
        mpsc::Sender<Result<(), DevelopmentError>>,
    ),
    Stop,
}

pub(super) struct Kqueue {
    excluded: Vec<PathBuf>,
    commands: mpsc::Sender<Command>,
    wake: std::io::PipeWriter,
    thread: Option<JoinHandle<()>>,
}

impl Kqueue {
    fn send(&mut self, command: Command) -> Result<(), DevelopmentError> {
        self.commands
            .send(command)
            .map_err(|e| DevelopmentError(format!("source watch stopped: {e}")))?;
        self.wake
            .write_all(&[0])
            .map_err(|e| DevelopmentError(format!("source watch wake failed: {e}")))
    }
}

impl SourceWatch for Kqueue {
    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> Result<(), DevelopmentError> {
        let walk = Walk::count(path, mode, &self.excluded)?;
        ensure_descriptors(path, &walk)?;
        let (reply, result) = mpsc::channel();
        self.send(Command::Add(path.to_path_buf(), mode, reply))?;
        result
            .recv()
            .map_err(|e| DevelopmentError(format!("source watch stopped: {e}")))?
    }
}

impl Drop for Kqueue {
    fn drop(&mut self) {
        if let Err(error) = self.send(Command::Stop) {
            tracing::error!(%error, "source watch stop failed");
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("source watch thread panicked");
        }
    }
}

pub(super) fn start(
    handler: Handler,
    excluded: Vec<PathBuf>,
) -> Result<Box<dyn SourceWatch>, DevelopmentError> {
    let (mut wake_reader, wake) =
        std::io::pipe().map_err(|e| DevelopmentError(format!("source watch pipe failed: {e}")))?;
    let mut watcher = kqueue::Watcher::new()
        .map_err(|e| DevelopmentError(format!("source watch queue failed: {e}")))?;
    let wake_fd = wake_reader.as_raw_fd();
    watcher
        .add_fd(wake_fd, EventFilter::EVFILT_READ, FilterFlag::empty())
        .and_then(|()| watcher.watch())
        .map_err(|e| DevelopmentError(format!("source watch queue failed: {e}")))?;
    let (commands, received) = mpsc::channel();
    let mut tree = Tree::new(excluded.clone());
    let thread = std::thread::Builder::new()
        .name("ssr-source-watch".into())
        .spawn(move || {
            let mut queue = Queue(watcher);
            loop {
                let Some(event) = queue.0.poll_forever(None) else {
                    handler(Err(notify::Error::generic("source watch queue stopped")));
                    return;
                };
                match (event.ident, event.data) {
                    (Ident::Fd(fd), EventData::ReadReady(_)) if fd == wake_fd => {
                        while let Ok(command) = received.try_recv() {
                            if let Err(error) = wake_reader.read_exact(&mut [0]) {
                                handler(Err(notify::Error::generic(&format!(
                                    "source watch wake failed: {error}"
                                ))));
                                return;
                            }
                            match command {
                                Command::Add(path, mode, reply) => {
                                    if reply.send(tree.add(&mut queue, &path, mode)).is_err() {
                                        tracing::error!("source watch registration receiver stopped");
                                    }
                                }
                                Command::Stop => return,
                            }
                        }
                    }
                    (Ident::Filename(_, path), EventData::Vnode(vnode)) => {
                        let change = match vnode {
                            Vnode::Write => Change::Write,
                            Vnode::Extend | Vnode::Truncate => Change::Size,
                            Vnode::Attrib => Change::Attributes,
                            Vnode::Link => Change::Link,
                            Vnode::Delete => Change::Delete,
                            Vnode::Rename => Change::Rename,
                            Vnode::Revoke => Change::Revoke,
                            other => {
                                handler(Err(notify::Error::generic(&format!(
                                    "source watch received an unexpected change {other:?} of {path}"
                                ))));
                                continue;
                            }
                        };
                        match tree.change(&mut queue, Path::new(&path), change) {
                            Ok(events) => events.into_iter().for_each(|event| handler(Ok(event))),
                            Err(error) => handler(Err(notify::Error::generic(&error.0))),
                        }
                    }
                    (ident, EventData::Error(error))
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        tracing::debug!(?ident, %error, "event of a descriptor that is no longer watched");
                    }
                    (ident, data) => handler(Err(notify::Error::generic(&format!(
                        "source watch received an unexpected event {data:?} of {ident:?}"
                    )))),
                }
            }
        })
        .map_err(|e| DevelopmentError(format!("source watch thread failed: {e}")))?;
    Ok(Box::new(Kqueue {
        excluded,
        commands,
        wake,
        thread: Some(thread),
    }))
}
