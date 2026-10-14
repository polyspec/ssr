//! Validate and execute the tests declared for shared workspace behavior (docs/test-ownership.md).
//!
//! The declaration `config/test-ownership.json` maps the public root names of every crate that another workspace
//! crate depends on to one owner test and one consumer test in every crate that uses them. Every location comes from
//! `cargo metadata`; Rust sources are read with `syn`, so comments and literals never count as uses. Run by
//!   cargo run --locked --manifest-path tests/test-ownership/Cargo.toml -- <repository root>

pub mod cargo_case;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use cargo_metadata::{Metadata, MetadataCommand, TargetKind};
use proc_macro2::{Spacing, TokenStream, TokenTree};
use serde::Deserialize;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, Item, Lit, Meta, UseTree, Visibility};

pub use cargo_case::{Cargo, Exit, Outcome};

/// The declaration, relative to the repository root.
pub const DECLARATION: &str = "config/test-ownership.json";

/// A failed check; its text is the report.
#[derive(Debug)]
pub struct OwnershipError(pub String);

impl OwnershipError {
    fn interrupted(signal: i32) -> Self {
        Self(format!("interrupted by signal {signal}"))
    }
}

impl fmt::Display for OwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for OwnershipError {}

impl From<io::Error> for OwnershipError {
    fn from(error: io::Error) -> Self {
        Self(error.to_string())
    }
}

/// The declaration file. Unknown and repeated fields of every object are rejected by the derived deserializers.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub behaviors: Vec<Behavior>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behavior {
    pub id: String,
    pub owner: CaseDeclaration,
    pub exports: Vec<String>,
    pub consumers: Vec<CaseDeclaration>,
}

/// One executable case: the package, its integration test target and the exact test function.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseDeclaration {
    #[serde(rename = "crate")]
    pub krate: String,
    pub target: String,
    pub test: String,
}

/// A validated case of a behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub behavior: String,
    pub krate: String,
    pub target: String,
    pub test: String,
}

pub fn parse_declaration(text: &str) -> Result<Declaration, OwnershipError> {
    serde_json::from_str(text)
        .map_err(|error| OwnershipError(format!("invalid declaration: {error}")))
}

/// A workspace member as `cargo metadata` describes it.
#[derive(Debug)]
pub struct Member {
    pub manifest_dir: PathBuf,
    /// The root file of the library target.
    pub lib: Option<PathBuf>,
    /// The root file of each integration test target, by target name.
    pub tests: BTreeMap<String, PathBuf>,
    /// The root file of every target: library, binaries, tests, examples, benchmarks and the build script.
    pub roots: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub members: BTreeMap<String, Member>,
    /// For each member, the Rust names of its path dependencies on other members and the package each one names.
    pub dependencies: BTreeMap<String, BTreeMap<String, String>>,
}

fn identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// A list in the report's notation: `['a', 'b']`.
fn listed<'a>(items: impl IntoIterator<Item = &'a String>) -> String {
    let items: Vec<String> = items.into_iter().map(|item| format!("'{item}'")).collect();
    format!("[{}]", items.join(", "))
}

fn elapsed(started: Instant) -> String {
    format!("{:.1}s", started.elapsed().as_secs_f64())
}

/// Read the workspace members. Cargo metadata can resolve the index, so it is a long operation: it has no time
/// limit, Cargo progress and errors reach standard error as they arrive, and the exit code decides the result.
pub fn workspace(
    root: &Path,
    cargo: &Cargo,
    out: &mut dyn Write,
) -> Result<Workspace, OwnershipError> {
    let root = fs::canonicalize(root)
        .map_err(|error| OwnershipError(format!("{}: {error}", root.display())))?;
    let arguments = [
        "metadata",
        "--no-deps",
        "--offline",
        "--format-version",
        "1",
    ];
    writeln!(out, "RUN cargo {}", arguments.join(" "))?;
    out.flush()?;
    let started = Instant::now();
    let mut command = cargo.command();
    command
        .args(arguments)
        .current_dir(&root)
        .stdout(Stdio::piped());
    let mut child = cargo_case::Supervised::spawn(&mut command)?;
    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout() {
        pipe.read_to_string(&mut stdout)?;
    }
    let exit = child.wait()?;
    if !exit.success() {
        writeln!(out, "FAIL cargo metadata exit {exit} {}", elapsed(started))?;
        out.flush()?;
        return Err(OwnershipError(format!(
            "workspace metadata failed with exit {exit}"
        )));
    }
    writeln!(out, "PASS cargo metadata {}", elapsed(started))?;
    out.flush()?;
    let metadata = MetadataCommand::parse(&stdout)
        .map_err(|error| OwnershipError(format!("unreadable workspace metadata: {error}")))?;
    members(root, &metadata)
}

fn members(root: PathBuf, metadata: &Metadata) -> Result<Workspace, OwnershipError> {
    let mut members = BTreeMap::new();
    for id in &metadata.workspace_members {
        let package = &metadata[id];
        let Some(manifest_dir) = package.manifest_path.parent() else {
            unreachable!("a manifest path names a file in a folder");
        };
        let mut lib = None;
        let mut tests = BTreeMap::new();
        let mut roots = Vec::new();
        for target in &package.targets {
            let path = PathBuf::from(&target.src_path);
            if target.kind.iter().any(|kind| {
                matches!(
                    kind,
                    TargetKind::Lib
                        | TargetKind::RLib
                        | TargetKind::DyLib
                        | TargetKind::CDyLib
                        | TargetKind::StaticLib
                        | TargetKind::ProcMacro
                )
            }) {
                lib = Some(path.clone());
            }
            if target.is_kind(TargetKind::Test) {
                tests.insert(target.name.clone(), path.clone());
            }
            roots.push(path);
        }
        let member = Member {
            manifest_dir: manifest_dir.into(),
            lib,
            tests,
            roots,
        };
        members.insert(package.name.to_string(), member);
    }
    let mut dependencies: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for id in &metadata.workspace_members {
        let package = &metadata[id];
        let aliases = dependencies.entry(package.name.to_string()).or_default();
        for dependency in &package.dependencies {
            if !members.contains_key(&dependency.name) || dependency.path.is_none() {
                continue;
            }
            let alias = dependency
                .rename
                .as_ref()
                .unwrap_or(&dependency.name)
                .replace('-', "_");
            // The same package as a normal and a development dependency has one Rust name.
            match aliases.get(&alias) {
                Some(target) if *target != dependency.name => {
                    return Err(OwnershipError(format!(
                        "ambiguous Rust dependency name in {}: {alias}",
                        package.name
                    )));
                }
                _ => {
                    aliases.insert(alias, dependency.name.clone());
                }
            }
        }
    }
    Ok(Workspace {
        root,
        members,
        dependencies,
    })
}

/// The lexical form of a path without `.` and `..` components.
fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    result
}

/// Fail on a symbolic link at a source file or at any folder between it and the repository root.
fn no_symbolic_link(root: &Path, file: &Path) -> Result<PathBuf, OwnershipError> {
    let file = normalized(file);
    if !file.starts_with(root) {
        return Err(OwnershipError(format!(
            "Rust source outside the repository: {}",
            file.display()
        )));
    }
    for ancestor in file.ancestors().take_while(|ancestor| *ancestor != root) {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|error| OwnershipError(format!("{}: {error}", ancestor.display())))?;
        if metadata.file_type().is_symlink() {
            return Err(OwnershipError(format!(
                "symbolic link in Rust source: {}",
                ancestor.display()
            )));
        }
    }
    Ok(file)
}

fn parse(path: &Path) -> Result<syn::File, OwnershipError> {
    let source = fs::read_to_string(path)
        .map_err(|error| OwnershipError(format!("{}: {error}", path.display())))?;
    syn::parse_file(&source).map_err(|error| {
        let start = error.span().start();
        OwnershipError(format!(
            "{}:{}:{}: {error}",
            path.display(),
            start.line,
            start.column + 1
        ))
    })
}

/// The value of a `#[path = "..."]` attribute.
fn path_attribute(attributes: &[syn::Attribute]) -> Option<String> {
    attributes
        .iter()
        .find_map(|attribute| match &attribute.meta {
            Meta::NameValue(pair) if pair.path.is_ident("path") => match &pair.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(value),
                    ..
                }) => Some(value.value()),
                _ => None,
            },
            _ => None,
        })
}

/// The Rust source files of a target: its root file and every module file that it declares, resolved as rustc
/// resolves `mod name;` and `#[path]` modules, each with its parsed syntax tree.
pub fn module_files(
    root: &Path,
    target_root: &Path,
    files: &mut BTreeMap<PathBuf, syn::File>,
) -> Result<(), OwnershipError> {
    let Some(folder) = target_root.parent() else {
        unreachable!("a target root is a file in a folder");
    };
    module_file(root, target_root, folder, files)
}

/// Read one module file whose child modules are in `children`.
fn module_file(
    root: &Path,
    path: &Path,
    children: &Path,
    files: &mut BTreeMap<PathBuf, syn::File>,
) -> Result<(), OwnershipError> {
    let path = no_symbolic_link(root, path)?;
    if files.contains_key(&path) {
        return Ok(());
    }
    let tree = parse(&path)?;
    let Some(folder) = path.parent() else {
        unreachable!("a module file is in a folder");
    };
    let mut declared = Vec::new();
    module_declarations(&tree.items, folder, children, true, &mut declared);
    files.insert(path.clone(), tree);
    for (file, folder) in declared {
        let (file, children) = match file {
            Ok(file) => (file, folder),
            Err(name) => {
                let flat = folder.join(format!("{name}.rs"));
                let nested = folder.join(&name).join("mod.rs");
                let file = match (flat.exists(), nested.exists()) {
                    (true, false) => flat,
                    (false, true) => nested,
                    (true, true) => {
                        return Err(OwnershipError(format!(
                            "{}: module {name} is both {} and {}",
                            path.display(),
                            flat.display(),
                            nested.display()
                        )));
                    }
                    (false, false) => {
                        return Err(OwnershipError(format!(
                            "{}: module {name} has no file {} or {}",
                            path.display(),
                            flat.display(),
                            nested.display()
                        )));
                    }
                };
                // The children of `name.rs` and of `name/mod.rs` are both in `name/`.
                (file, folder.join(&name))
            }
        };
        module_file(root, &file, &children, files)?;
    }
    Ok(())
}

/// Collect the `mod name;` declarations of items with the folder of their children: `Ok(file)` for a `#[path]`
/// module, `Err(name)` for a module found by its name in the folder. `#[path]` is relative to the file's folder at the top level and to the inline module's
/// folder inside one.
fn module_declarations(
    items: &[Item],
    file_folder: &Path,
    children: &Path,
    top: bool,
    declared: &mut Vec<(Result<PathBuf, String>, PathBuf)>,
) {
    for item in items {
        let Item::Mod(module) = item else {
            continue;
        };
        let name = module.ident.to_string();
        let explicit = path_attribute(&module.attrs);
        match &module.content {
            None => match explicit {
                Some(explicit) => {
                    let base = if top { file_folder } else { children };
                    let file = base.join(explicit);
                    // A `#[path]` file is read as a `mod.rs` file: its children are in its own folder.
                    let folder = file.parent().map(Path::to_path_buf).unwrap_or_default();
                    declared.push((Ok(file), folder));
                }
                None => declared.push((Err(name), children.to_path_buf())),
            },
            Some((_, inner)) => {
                let folder = match explicit {
                    Some(explicit) => children.join(explicit),
                    None => children.join(&name),
                };
                module_declarations(inner, file_folder, &folder, false, declared);
            }
        }
    }
}

/// The public names at a library's crate root.
pub fn public_root_exports(lib: &Path) -> Result<BTreeSet<String>, OwnershipError> {
    let tree = parse(lib)?;
    let mut exports = BTreeSet::new();
    for item in &tree.items {
        let (visibility, name) = match item {
            Item::Use(item) => {
                if matches!(item.vis, Visibility::Public(_)) {
                    public_use_names(&item.tree, &[], &mut exports)
                        .map_err(|error| OwnershipError(format!("{}: {error}", lib.display())))?;
                }
                continue;
            }
            Item::Const(item) => (&item.vis, &item.ident),
            Item::Enum(item) => (&item.vis, &item.ident),
            Item::ExternCrate(item) => (
                &item.vis,
                item.rename
                    .as_ref()
                    .map_or(&item.ident, |(_, rename)| rename),
            ),
            Item::Fn(item) => (&item.vis, &item.sig.ident),
            Item::Mod(item) => (&item.vis, &item.ident),
            Item::Static(item) => (&item.vis, &item.ident),
            Item::Struct(item) => (&item.vis, &item.ident),
            Item::Trait(item) => (&item.vis, &item.ident),
            Item::TraitAlias(item) => (&item.vis, &item.ident),
            Item::Type(item) => (&item.vis, &item.ident),
            Item::Union(item) => (&item.vis, &item.ident),
            // Implementations, foreign blocks and macro_rules have no visibility of their own.
            Item::Impl(_) | Item::ForeignMod(_) | Item::Macro(_) => continue,
            Item::Verbatim(tokens) => {
                if matches!(tokens.clone().into_iter().next(), Some(TokenTree::Ident(word)) if word == "pub")
                {
                    return Err(OwnershipError(format!(
                        "unrecognized public root item in {}",
                        lib.display()
                    )));
                }
                continue;
            }
            _ => {
                return Err(OwnershipError(format!(
                    "unrecognized root item in {}",
                    lib.display()
                )));
            }
        };
        if matches!(visibility, Visibility::Public(_)) && name != "_" {
            exports.insert(name.to_string());
        }
    }
    if exports.is_empty() {
        return Err(OwnershipError(format!(
            "no public root exports found in {}",
            lib.display()
        )));
    }
    Ok(exports)
}

fn public_use_names(
    tree: &UseTree,
    prefix: &[String],
    names: &mut BTreeSet<String>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut prefix = prefix.to_vec();
            prefix.push(path.ident.to_string());
            public_use_names(&path.tree, &prefix, names)
        }
        UseTree::Name(name) if name.ident == "self" => {
            if let Some(last) = prefix.last() {
                names.insert(last.clone());
            }
            Ok(())
        }
        UseTree::Name(name) => {
            names.insert(name.ident.to_string());
            Ok(())
        }
        UseTree::Rename(rename) => {
            // `as _` imports a trait for its methods and exports no name.
            if rename.rename != "_" {
                names.insert(rename.rename.to_string());
            }
            Ok(())
        }
        UseTree::Glob(_) => {
            Err("public root glob import cannot be mapped to one public name".to_owned())
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .try_for_each(|item| public_use_names(item, prefix, names)),
    }
}

/// The paths that a `use` tree selects, each from its first segment; `self` in a group selects the group's path,
/// renames are dropped and a glob is the segment `*`.
pub fn import_paths(tree: &UseTree) -> Vec<Vec<String>> {
    let mut paths = Vec::new();
    collect_import_paths(tree, &mut Vec::new(), &mut paths);
    paths
}

fn collect_import_paths(tree: &UseTree, prefix: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_import_paths(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(syn::UseName { ident }) | UseTree::Rename(syn::UseRename { ident, .. }) => {
            let mut path = prefix.clone();
            if ident != "self" {
                path.push(ident.to_string());
            }
            paths.push(path);
        }
        UseTree::Glob(_) => {
            let mut path = prefix.clone();
            path.push("*".to_owned());
            paths.push(path);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_import_paths(item, prefix, paths);
            }
        }
    }
}

/// The public root names of declared owners that one source file uses, through `use` trees, qualified paths and
/// the tokens of macro invocations and attribute arguments.
struct References<'a> {
    aliases: &'a BTreeMap<String, String>,
    exports: &'a BTreeMap<String, BTreeSet<String>>,
    found: BTreeSet<(String, String)>,
    errors: Vec<String>,
}

impl References<'_> {
    fn import(&mut self, tree: &UseTree) {
        for path in import_paths(tree) {
            let Some(owner) = path.first().and_then(|first| self.aliases.get(first)) else {
                continue;
            };
            if path.iter().any(|segment| segment == "*") {
                self.errors.push(format!(
                    "wildcard import from declared owner is unsupported: {}",
                    path.join("::")
                ));
                continue;
            }
            let Some(name) = path.get(1) else {
                self.errors.push(format!(
                    "crate-root import alias is unsupported: {}",
                    path[0]
                ));
                continue;
            };
            let owner = owner.clone();
            self.name(&owner, name, "import");
        }
    }

    fn name(&mut self, owner: &str, name: &str, kind: &str) {
        if self
            .exports
            .get(owner)
            .is_some_and(|exports| exports.contains(name))
        {
            self.found.insert((owner.to_owned(), name.to_owned()));
        } else {
            self.errors.push(format!(
                "unrecognized public name in {kind}: {owner}::{name}"
            ));
        }
    }

    /// Paths `alias::Name` in unparsed tokens; `alias::{...}` is read as a `use` tree.
    fn tokens(&mut self, tokens: TokenStream) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        for (index, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(group) => self.tokens(group.stream()),
                TokenTree::Ident(ident) => {
                    let alias = ident.to_string();
                    let Some(owner) = self.aliases.get(&alias).cloned() else {
                        continue;
                    };
                    let separated = matches!(
                        (trees.get(index + 1), trees.get(index + 2)),
                        (Some(TokenTree::Punct(first)), Some(TokenTree::Punct(second)))
                            if first.as_char() == ':' && first.spacing() == Spacing::Joint && second.as_char() == ':'
                    );
                    if !separated {
                        continue;
                    }
                    match trees.get(index + 3) {
                        Some(TokenTree::Ident(name)) => {
                            self.name(&owner, &name.to_string(), "reference")
                        }
                        Some(TokenTree::Group(_)) => {
                            let tokens: TokenStream =
                                trees[index..index + 4].iter().cloned().collect();
                            match syn::parse2::<UseTree>(tokens) {
                                Ok(tree) => self.import(&tree),
                                Err(error) => self
                                    .errors
                                    .push(format!("unrecognized use tree of {alias}: {error}")),
                            }
                        }
                        Some(TokenTree::Punct(star)) if star.as_char() == '*' => {
                            self.errors
                                .push(format!("wildcard reference to declared owner: {alias}"));
                        }
                        _ => {}
                    }
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for References<'_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.import(&item.tree);
        visit::visit_item_use(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let mut segments = path.segments.iter();
        if let (Some(first), Some(second)) = (segments.next(), segments.next())
            && let Some(owner) = self.aliases.get(&first.ident.to_string()).cloned()
        {
            self.name(&owner, &second.ident.to_string(), "reference");
        }
        visit::visit_path(self, path);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.tokens(mac.tokens.clone());
        visit::visit_macro(self, mac);
    }

    fn visit_meta_list(&mut self, list: &'ast syn::MetaList) {
        self.tokens(list.tokens.clone());
        visit::visit_meta_list(self, list);
    }
}

/// The owner names that a parsed source file uses.
pub fn references(
    tree: &syn::File,
    aliases: &BTreeMap<String, String>,
    exports: &BTreeMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<(String, String)>, String> {
    let mut visitor = References {
        aliases,
        exports,
        found: BTreeSet::new(),
        errors: Vec::new(),
    };
    visitor.visit_file(tree);
    match visitor.errors.into_iter().next() {
        Some(error) => Err(error),
        None => Ok(visitor.found),
    }
}

/// A function at the root of a test target that runs as the named case: `#[test]` or `#[tokio::test]`, no
/// parameters.
fn is_test(function: &syn::ItemFn, name: &str) -> bool {
    function.sig.ident == name
        && function.sig.inputs.is_empty()
        && function.attrs.iter().any(|attribute| {
            let segments: Vec<String> = attribute
                .path()
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            segments == ["test"] || segments == ["tokio", "test"]
        })
}

fn case(workspace: &Workspace, behavior: &str, value: &CaseDeclaration) -> Result<Case, String> {
    let CaseDeclaration {
        krate,
        target,
        test,
    } = value;
    let Some(member) = workspace
        .members
        .get(krate)
        .filter(|_| identifier(target) && identifier(test))
    else {
        return Err(format!("{behavior}: invalid crate, target or test"));
    };
    let Some(path) = member.tests.get(target) else {
        return Err(format!(
            "{behavior}: missing test target in {krate}: {target}"
        ));
    };
    let path =
        no_symbolic_link(&workspace.root, path).map_err(|error| format!("{behavior}: {error}"))?;
    let tree = parse(&path).map_err(|error| format!("{behavior}: {error}"))?;
    let functions: Vec<&syn::ItemFn> = tree
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if is_test(function, test) => Some(function),
            _ => None,
        })
        .collect();
    let [function] = functions.as_slice() else {
        return Err(format!(
            "{behavior}: missing or repeated test {krate}::{target} {test}"
        ));
    };
    if function.block.stmts.is_empty() {
        return Err(format!("{behavior}: empty test {krate}::{target} {test}"));
    }
    Ok(Case {
        behavior: behavior.to_owned(),
        krate: krate.clone(),
        target: target.clone(),
        test: test.clone(),
    })
}

/// Check the declaration against the workspace and return its cases. Every declaration error is collected and
/// reported together in one error, so one run names them all.
pub fn validate(
    root: &Path,
    declaration: &Declaration,
    cargo: &Cargo,
    out: &mut dyn Write,
) -> Result<Vec<Case>, OwnershipError> {
    let workspace = workspace(root, cargo, out)?;
    if declaration.behaviors.is_empty() {
        return Err(OwnershipError(
            "at least one behavior is required".to_owned(),
        ));
    }
    let mut errors = Vec::new();
    let mut cases = Vec::new();
    let mut seen = BTreeSet::new();
    let mut valid: Vec<&Behavior> = Vec::new();
    for behavior in &declaration.behaviors {
        let name = &behavior.id;
        if !identifier(name) || !seen.insert(name.clone()) {
            errors.push(format!("invalid or repeated behavior: {name}"));
            continue;
        }
        let owner = match case(&workspace, name, &behavior.owner) {
            Ok(owner) => owner,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let exported = &behavior.exports;
        if exported.is_empty()
            || !exported.iter().all(|item| identifier(item))
            || exported.iter().collect::<BTreeSet<_>>().len() != exported.len()
        {
            errors.push(format!(
                "{name}: exports must be a nonempty list of unique public names"
            ));
            continue;
        }
        let mut consumers = Vec::new();
        for item in &behavior.consumers {
            match case(&workspace, name, item) {
                Ok(consumer) => consumers.push(consumer),
                Err(error) => errors.push(error),
            }
        }
        let crates: BTreeSet<&String> = behavior.consumers.iter().map(|item| &item.krate).collect();
        if crates.len() != behavior.consumers.len() {
            errors.push(format!("{name}: repeated consumer crate"));
            continue;
        }
        cases.push(owner);
        cases.extend(consumers);
        valid.push(behavior);
    }

    let depended: BTreeSet<&String> = workspace
        .dependencies
        .values()
        .flat_map(BTreeMap::values)
        .collect();
    let declared_owners: BTreeSet<&String> =
        valid.iter().map(|behavior| &behavior.owner.krate).collect();
    for crate_name in depended {
        if !declared_owners.contains(crate_name) {
            let consumers = workspace
                .dependencies
                .iter()
                .filter(|(_, aliases)| aliases.values().any(|target| target == crate_name))
                .map(|(name, _)| name);
            errors.push(format!(
                "{crate_name} is a dependency of {} but owns no declared behavior",
                listed(consumers)
            ));
        }
    }

    let mut owner_exports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut failed_owners = BTreeSet::new();
    let mut behavior_for_export: BTreeMap<(String, String), String> = BTreeMap::new();
    for behavior in &valid {
        let owner_name = &behavior.owner.krate;
        if failed_owners.contains(owner_name) {
            continue;
        }
        if !owner_exports.contains_key(owner_name) {
            let member = &workspace.members[owner_name];
            let exports = match &member.lib {
                None => Err(OwnershipError(format!(
                    "{owner_name} has no library target"
                ))),
                Some(lib) => {
                    no_symbolic_link(&workspace.root, lib).and_then(|lib| public_root_exports(&lib))
                }
            };
            match exports {
                Ok(exports) => {
                    owner_exports.insert(owner_name.clone(), exports);
                }
                Err(error) => {
                    errors.push(error.0);
                    failed_owners.insert(owner_name.clone());
                    continue;
                }
            }
        }
        for exported in &behavior.exports {
            if !owner_exports[owner_name].contains(exported) {
                errors.push(format!(
                    "{}: owner does not export {owner_name}::{exported}",
                    behavior.id
                ));
                continue;
            }
            let key = (owner_name.clone(), exported.clone());
            if behavior_for_export.contains_key(&key) {
                errors.push(format!(
                    "public root name is assigned more than once: {owner_name}::{exported}"
                ));
                continue;
            }
            behavior_for_export.insert(key, behavior.id.clone());
        }
    }
    for (owner_name, exports) in &owner_exports {
        let declared: BTreeSet<&String> = behavior_for_export
            .keys()
            .filter(|(owner, _)| owner == owner_name)
            .map(|(_, name)| name)
            .collect();
        if !exports.iter().eq(declared.iter().copied()) {
            errors.push(format!(
                "{owner_name}: declared public names {} differ from root exports {}",
                listed(declared),
                listed(exports)
            ));
        }
    }

    let mut actual_consumers: BTreeMap<&String, BTreeSet<&String>> = valid
        .iter()
        .map(|behavior| (&behavior.id, BTreeSet::new()))
        .collect();
    for (crate_name, aliases) in &workspace.dependencies {
        let aliases: BTreeMap<String, String> = aliases
            .iter()
            .filter(|(_, target)| owner_exports.contains_key(*target))
            .map(|(alias, target)| (alias.clone(), target.clone()))
            .collect();
        if aliases.is_empty() {
            continue;
        }
        let mut files = BTreeMap::new();
        for target_root in &workspace.members[crate_name].roots {
            if let Err(error) = module_files(&workspace.root, target_root, &mut files) {
                errors.push(error.0);
            }
        }
        for (path, tree) in &files {
            let found = match references(tree, &aliases, &owner_exports) {
                Ok(found) => found,
                Err(error) => {
                    errors.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            for key in found {
                match behavior_for_export.get(&key) {
                    Some(behavior) => {
                        if let Some(consumers) = actual_consumers.get_mut(behavior) {
                            consumers.insert(crate_name);
                        }
                    }
                    None => errors.push(format!("unmapped public name: {}::{}", key.0, key.1)),
                }
            }
        }
    }

    for behavior in &valid {
        let declared: BTreeSet<&String> =
            behavior.consumers.iter().map(|item| &item.krate).collect();
        let actual = &actual_consumers[&behavior.id];
        if declared != *actual {
            errors.push(format!(
                "{}: declared consumer crates {} differ from Rust source consumers {}",
                behavior.id,
                listed(declared),
                listed(actual.iter().copied())
            ));
        }
    }
    if !errors.is_empty() {
        let mut unique = Vec::new();
        for error in errors {
            if !unique.contains(&error) {
                unique.push(error);
            }
        }
        let lines: Vec<String> = unique.iter().map(|error| format!("  {error}")).collect();
        return Err(OwnershipError(format!(
            "{} declaration errors:\n{}",
            unique.len(),
            lines.join("\n")
        )));
    }
    Ok(cases)
}

/// Runs `cargo <args>` and returns its exit and output; the check's own runner is [`cargo_case::run`].
pub type CargoRun<'a> = dyn FnMut(&[&str], &mut dyn Write) -> Result<Outcome, OwnershipError> + 'a;

/// Build every workspace test once, then run the declared cases on those builds.
///
/// A separate `-p` run for each case resolves features for that package alone, so shared crates are compiled
/// again for each feature set. One workspace build matches the later workspace test run, and one nextest run
/// executes every case with its own nextest limit, which only ends a hung case. The build and the run are long
/// operations with no time limit.
pub fn run(
    cases: &[Case],
    command: &mut CargoRun<'_>,
    out: &mut dyn Write,
) -> Result<(), OwnershipError> {
    let build = [
        "nextest",
        "run",
        "--workspace",
        "--locked",
        "--no-run",
        "--color",
        "never",
    ];
    writeln!(out, "RUN build workspace tests")?;
    out.flush()?;
    let started = Instant::now();
    let result = command(&build, out)?;
    if !result.exit.success() {
        return Err(OwnershipError(format!(
            "build workspace tests: failed (exit {})\n{}",
            result.exit, result.output
        )));
    }
    writeln!(out, "PASS build workspace tests {}", elapsed(started))?;
    let expression = cases
        .iter()
        .map(|item| {
            format!(
                "(package(={}) & binary(={}) & test(={}))",
                item.krate, item.target, item.test
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    let mut args = vec![
        "nextest",
        "run",
        "--workspace",
        "--locked",
        "--no-tests",
        "fail",
        "--color",
        "never",
    ];
    args.extend(cargo_case::REPORT);
    args.extend(["-E", &expression]);
    writeln!(out, "RUN {} ownership cases", cases.len())?;
    out.flush()?;
    let started = Instant::now();
    let result = command(&args, out)?;
    let events = cargo_case::results(&result.output)?;
    let mut missing = Vec::new();
    for item in cases {
        let label = format!(
            "{}: {}::{} {}",
            item.behavior, item.krate, item.target, item.test
        );
        let event = events
            .get(&format!("{}::{}${}", item.krate, item.target, item.test))
            .copied()
            .unwrap_or("not run");
        if event == "ok" {
            writeln!(out, "PASS {label}")?;
        } else {
            writeln!(out, "FAIL {label} ({event})")?;
            missing.push(label);
        }
        out.flush()?;
    }
    if !result.exit.success() || !missing.is_empty() {
        return Err(OwnershipError(format!(
            "ownership cases {} (exit {}): {}\n{}",
            if result.exit.success() {
                "did not pass"
            } else {
                "failed"
            },
            result.exit,
            missing.join(", "),
            result.output
        )));
    }
    writeln!(
        out,
        "PASS {} ownership cases {}",
        cases.len(),
        elapsed(started)
    )?;
    out.flush()?;
    Ok(())
}

/// The whole check of a repository: read the declaration, validate it and run its cases with Cargo.
pub fn check(root: &Path, cargo: &Cargo, out: &mut dyn Write) -> Result<usize, OwnershipError> {
    let path = root.join(DECLARATION);
    let text = fs::read_to_string(&path)
        .map_err(|error| OwnershipError(format!("{}: {error}", path.display())))?;
    let declaration = parse_declaration(&text)?;
    let cases = validate(root, &declaration, cargo, out)?;
    run(
        &cases,
        &mut |args, out| cargo_case::run(cargo, root, args, out),
        out,
    )?;
    Ok(cases.len())
}
