use super::*;

/// A kernel that reports a change only for a registered path; a change of a path before its
/// registration is lost, as `kqueue(2)` cannot report a vnode that it does not watch.
#[derive(Default)]
struct Fake {
    registered: BTreeSet<PathBuf>,
    lost: Vec<PathBuf>,
}

impl Kernel for Fake {
    fn register(&mut self, paths: &[PathBuf]) -> Result<Vec<PathBuf>, DevelopmentError> {
        let present = paths
            .iter()
            .filter(|path| std::fs::symlink_metadata(path).is_ok())
            .cloned()
            .collect::<Vec<_>>();
        self.registered.extend(present.iter().cloned());
        Ok(present)
    }
    fn unregister(&mut self, path: &Path) -> Result<(), DevelopmentError> {
        assert!(
            self.registered.remove(path),
            "{} was not registered",
            path.display()
        );
        Ok(())
    }
}

/// Delivers a change as the kernel would: only for a registered path.
fn deliver(tree: &mut Tree, kernel: &mut Fake, path: &Path, change: Change) -> Vec<Event> {
    if !kernel.registered.contains(path) {
        kernel.lost.push(path.to_path_buf());
        return Vec::new();
    }
    tree.change(kernel, path, change).unwrap()
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).unwrap();
        let name = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("ssr-kqueue-{name}"));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn kinds(events: &[Event]) -> Vec<(EventKind, PathBuf)> {
    events
        .iter()
        .map(|event| (event.kind, event.paths[0].clone()))
        .collect()
}

#[test]
fn a_write_to_a_file_created_after_its_directory_event_is_observed() {
    let root = Directory::new();
    let source = root.0.join("source");
    std::fs::create_dir(&source).unwrap();
    let mut tree = Tree::new(Vec::new());
    let mut kernel = Fake::default();
    tree.add(&mut kernel, &source, RecursiveMode::Recursive)
        .unwrap();
    let file = source.join("page.js");
    std::fs::write(&file, "first").unwrap();
    assert!(deliver(&mut tree, &mut kernel, &file, Change::Write).is_empty());
    assert_eq!(
        kernel.lost,
        vec![file.clone()],
        "the write before registration"
    );
    let created = deliver(&mut tree, &mut kernel, &source, Change::Write);
    assert_eq!(
        kinds(&created),
        vec![(EventKind::Create(CreateKind::File), file.clone())]
    );
    assert!(
        kernel.registered.contains(&file),
        "the creation is reported only after the file is registered"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "first",
        "the build that the creation starts reads the write that the kernel lost"
    );
    std::fs::write(&file, "second").unwrap();
    assert_eq!(
        kinds(&deliver(&mut tree, &mut kernel, &file, Change::Write)),
        vec![(
            EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            file
        )]
    );
}

#[test]
fn a_directory_created_with_entries_is_registered_before_its_creation_is_reported() {
    let root = Directory::new();
    let source = root.0.join("source");
    std::fs::create_dir(&source).unwrap();
    let mut tree = Tree::new(Vec::new());
    let mut kernel = Fake::default();
    tree.add(&mut kernel, &source, RecursiveMode::Recursive)
        .unwrap();
    let nested = source.join("pages");
    let inner = nested.join("deep");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join("page.js"), "first").unwrap();
    let created = deliver(&mut tree, &mut kernel, &source, Change::Write);
    assert_eq!(
        kinds(&created),
        vec![
            (EventKind::Create(CreateKind::Folder), nested.clone()),
            (EventKind::Create(CreateKind::Folder), inner.clone()),
            (EventKind::Create(CreateKind::File), inner.join("page.js")),
        ]
    );
    for path in [&nested, &inner, &inner.join("page.js")] {
        assert!(kernel.registered.contains(path), "{}", path.display());
    }
}

#[test]
fn excluded_paths_are_never_registered() {
    let root = Directory::new();
    let source = root.0.join("source");
    let modules = source.join("node_modules");
    std::fs::create_dir_all(modules.join("react")).unwrap();
    std::fs::write(source.join("page.js"), "page").unwrap();
    let mut tree = Tree::new(vec![modules.clone()]);
    let mut kernel = Fake::default();
    tree.add(&mut kernel, &source, RecursiveMode::Recursive)
        .unwrap();
    assert_eq!(
        kernel.registered,
        BTreeSet::from([source.clone(), source.join("page.js")])
    );
    std::fs::create_dir(modules.join("vue")).unwrap();
    std::fs::write(modules.join("vue/index.js"), "vue").unwrap();
    assert!(deliver(&mut tree, &mut kernel, &modules, Change::Write).is_empty());
    assert_eq!(kernel.lost, vec![modules]);
    let walk = Walk::count(&source, RecursiveMode::Recursive, &tree.excluded).unwrap();
    assert_eq!(walk.paths, 2);
}

#[test]
fn removed_entries_are_unregistered_and_reported() {
    let root = Directory::new();
    let source = root.0.join("source");
    let nested = source.join("pages");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("page.js"), "page").unwrap();
    let mut tree = Tree::new(Vec::new());
    let mut kernel = Fake::default();
    tree.add(&mut kernel, &source, RecursiveMode::Recursive)
        .unwrap();
    std::fs::remove_dir_all(&nested).unwrap();
    let removed = deliver(&mut tree, &mut kernel, &source, Change::Write);
    assert_eq!(
        kinds(&removed),
        vec![(EventKind::Remove(RemoveKind::Any), nested.clone())]
    );
    assert_eq!(kernel.registered, BTreeSet::from([source]));
}

#[test]
fn a_nonrecursive_directory_registers_its_files_only() {
    let root = Directory::new();
    let parent = root.0.join("config");
    std::fs::create_dir_all(parent.join("nested")).unwrap();
    std::fs::write(parent.join("environment.json"), "{}").unwrap();
    let mut tree = Tree::new(Vec::new());
    let mut kernel = Fake::default();
    tree.add(&mut kernel, &parent, RecursiveMode::NonRecursive)
        .unwrap();
    assert_eq!(
        kernel.registered,
        BTreeSet::from([parent.clone(), parent.join("environment.json")])
    );
    let walk = Walk::count(&parent, RecursiveMode::NonRecursive, &[]).unwrap();
    assert_eq!(walk.paths, 2);
}

#[test]
fn the_descriptor_limit_is_kept_raised_or_reported() {
    assert_eq!(descriptor_limit(100, 50, 256, Some(1024)), Ok(None));
    assert_eq!(descriptor_limit(300, 50, 256, Some(1024)), Ok(Some(556)));
    assert_eq!(descriptor_limit(300, 50, 256, None), Ok(Some(556)));
    assert_eq!(descriptor_limit(900, 50, 256, Some(1000)), Ok(Some(1000)));
    assert_eq!(descriptor_limit(990, 50, 256, Some(1000)), Err(1000));
}

#[test]
fn the_walk_names_its_largest_directories() {
    let root = Directory::new();
    let source = root.0.join("source");
    let pages = source.join("pages");
    std::fs::create_dir_all(&pages).unwrap();
    for index in 0..3 {
        std::fs::write(pages.join(format!("{index}.js")), "page").unwrap();
    }
    std::fs::write(source.join("app.js"), "app").unwrap();
    let walk = Walk::count(&source, RecursiveMode::Recursive, &[]).unwrap();
    assert_eq!(walk.paths, 6);
    assert_eq!(
        walk.largest(),
        format!("{} (3), {} (2)", pages.display(), source.display())
    );
}
