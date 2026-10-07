[Korean](changelog.ko.md)

# Changelog

## Unreleased

## 0.0.2

- Depend on the released tag `v0.0.2` of ordered-json, version `=0.0.2`, in the
  workspace manifest, `Cargo.lock` and `verification/engine/Cargo.lock`. The tag
  `v0.0.1` of ordered-json has no GitHub Release.

- Link the changelog from the release notes when the section `## X.Y.Z`
  exceeds 125000 characters, the limit of a GitHub release body:
  `make release-publish` then passes one line that links the section
  `## X.Y.Z` of `docs/changelog.md` at the tag.

## 0.0.1

- Verify Linux on the Linux runners of CI only. `make check` runs no Linux
  container: the jobs `lint` and `test` of `.github/workflows/ci.yml` lint,
  build and test the workspace on `ubuntu-24.04-arm` and `ubuntu-24.04` with
  `check-clippy`, `check-examples` and `ci-nextest`. The targets `check-linux`,
  `verify-engine-linux-arm64` and `verify-engine-down`, their tools and tests,
  the engine Compose stack and the declarations of the host tools `container`
  and `containerctl` are removed.

- Run the CI targets in two jobs on each architecture: the job `lint` of
  `.github/workflows/ci.yml` runs `make ci-lint` (`ci-records`, `check-fmt`,
  `check-clippy`, `check-deny`) and the job `test` runs `make ci-test`
  (`check-examples`, `ci-nextest`), each CI target in exactly one job, and
  `ci-passed` needs both.

- Install cargo-nextest 0.9.146 and cargo-deny 0.20.2 in CI from their
  official release archives instead of compiling them. `tools/tool-versions.json`
  declares the URL, SHA-256 and executable path of each archive under
  `releases`, `tools/fetch_tools.py` installs only a verified executable, and
  `make ci` checks the version report of each tool.

- Build the CI targets with one Cargo job per processor of the runner.
  `BUILD_JOBS ?= 1` sets `CARGO_BUILD_JOBS` of the host targets, so a local
  build runs one job, and `make ci` sets `BUILD_JOBS` to the output of `nproc`.

- Release the tags `<directory>/vX.Y.Z` at any depth and only npm and Composer
  archives. The trigger of `.github/workflows/release.yml` is
  `tags: ['v*', '**/v*']`: in a tag filter `*` does not match `/`. A crate is
  not released as an archive; it is consumed by git tag, because
  `cargo package` rewrites git dependencies into crates.io requirements that
  do not resolve. `tools/release.py` lists every crate manifest so, and
  `make release-setup` checks Python only.

- Release a tag of a commit of `main` through `.github/workflows/release.yml`,
  which runs on the push of a tag `v*` or `**/v*` with the permission
  `contents: write`. `make release-verify` requires the tagged commit on
  `origin/main` with the check runs `push-gate` and `ci-passed` concluded
  `success`, `make release-versions` the version of the tag in every manifest
  and the section `## X.Y.Z` in `docs/changelog.md`, `make release-assets`
  builds the npm and Composer archives, of which the repository has none, and
  `make release-publish` creates the GitHub Release with the section as notes
  (`tools/release.py`, `tools/test_release.py`). AGENTS states the release
  procedure.

- Require exactly the checks `push-gate` and `ci-passed` in the ruleset
  `main`. `ci-passed`, the last job of `.github/workflows/ci.yml`, needs
  every other job, runs after each of them under `if: ${{ always() }}` and
  runs `make ci-passed`, which fails unless every needed job has the result
  `success`; a job added to `ci.yml` is required once it is in `needs`.

- Keep the section `## Unreleased` at the top of the changelog. No version
  of ssr is released, so every entry is under it; every change adds its
  entry there, and a release renames it to `## X.Y.Z` below a new empty
  `## Unreleased`.

- Run `.github/workflows/ci.yml` on every pull request, merge group and manual
  run (`workflow_dispatch`), and declare the triggers of each workflow exactly.

- Depend on the tag `v0.0.1` of ordered-json in the workspace manifest,
  `Cargo.lock` and `verification/engine/Cargo.lock`.

- Lock ordered-json at the current commit of the `main` branch of its
  repository in `Cargo.lock` and `verification/engine/Cargo.lock`.

- The crates are `polyspec-ssr-core`, `polyspec-ssr-build`, `polyspec-ssr-runtime`,
  `polyspec-ssr-adapter-react`, `polyspec-ssr-adapter-vue`, `polyspec-ssr-adapter-svelte`,
  `polyspec-ssr-adapter-vanilla`, `polyspec-ssr-nonce` and `polyspec-ssr-server`, with the
  libraries `polyspec_ssr_core` and so on.

- Depend on ordered-json as the package `polyspec-ordered-json` of its Git
  repository, named without a commit; `Cargo.lock` records the resolved commit.

- `python3 -m tools.owning_tests select` reads only the changed files that
  exist; a removed file is not a changed file.

- Check the tools of a full run against `tools/tool-versions.json` of the
  checkout that runs, so the guard cases of a temporary checkout no longer
  depend on the executables of the host.

- Run the browser cases with the chromium build that playwright-core pins,
  installed with the test packages, instead of the host's browser.

- Build and test `ssr` on Linux in GitHub CI: an arm64 and an x86_64 job run
  only make targets, run every check past failures and upload the per-target
  logs and a summary with the first failure lines; the push check job uploads
  its logs the same way.

- Send an interruption to every process of a full run step, so an interrupted
  `make check` also stops the cargo that a step's tool started.

- Build from published and pinned dependency sources only: v8 150.4.0 from
  crates.io with the official archive, lightningcss from a pinned Git commit of
  its fork, ordered-json from its Git repository, and a check that fails for an absolute path into a home directory
  in any tracked file.

- Require an absolute dependency directory without symbolic links:
  `BuildConfig::dependencies` follows the rule of the root.

- Name the asset plugin's error in a failed JavaScript build: a rejected or
  unreadable asset now reports its cause next to the rolldown error, which
  drops the plugin's message.

- Judge the browser launch cases without a browser: a fake launch records its
  options and refuses a launch limit, so the cases no longer depend on how fast
  the host pages a real browser in.

- Remove the symbolic links of the React dependency case: it copies the
  packages it needs into its own root.

- Export the host engine inputs to host targets only: `make check-linux` no
  longer receives the host V8 archive and passes its Linux V8 step.

- Give each run its own build-probe packages and generated entries: the
  packages are installed once per lock into an immutable directory published by
  one rename, each test builds in a temporary root of its own with that
  installation as its dependencies, and nothing writes into the checkout's
  fixture directory.

- Build the assets of a configured dependency package: a CSS or JavaScript
  asset may be a file under the application root or under the package
  directory, so a package configured with `BuildConfig::dependencies` ships its
  own fonts and images; an asset outside both is still rejected.

- Run the push check on GitHub with the declared Python: Python is pinned to
  its minor version 3.9, the check prints the running patch release, and the
  workflow installs Python 3.9 on `ubuntu-24.04-arm` with actions pinned to
  commits and checks it before the push check.

- Map every changed file to its owning tests: every crate that another crate
  depends on has a declared behavior with owner and consumer tests, and
  `tools/test-owners.json` names the owning tests of every tracked file, which
  `python3 -m tools.owning_tests select` prints for a change.

- Report every check of one run: a failed setup step of `make check` skips
  only the targets that read its output, `make verify-build` runs each of its
  checks, and the ownership check reports every declaration error.

- Judge tool results by stable output: the ownership and feature checks read
  the libtest-json report of nextest, the engine mount check judges access
  modes only, and the browser and archive cases no longer read browser error
  text or `make --dry-run` output.

- Make each failure name its cause: the benchmark runs the executable that
  its build reports, and the engine status, archive, record and runtime child
  checks name the command output and the expected and actual values.

- Judge the runtime cancellation cases by events: the unread stream case
  waits for the worker to return its capacity instead of sleeping, and normal
  renders in these cases are no longer bounded by 300 ms.

- Remove the time, the registry and unpinned tools from the results:
  `make check` checks dependency bans, licenses and sources, and
  `make review-advisories` reviews security advisories separately; the license case reads tracked locks; rustup no longer installs a
  missing toolchain; and `tools/tool-versions.json` declares every tool, which
  each entry checks before its first step.

- Run the example programs that the test build compiled: a nextest setup
  script builds the `development_process` and `socket_process` examples before
  any test of `polyspec-ssr-server` and names them in `SSR_DEVELOPMENT_PROCESS` and
  `SSR_SOCKET_PROCESS`, so a run of one test target no longer runs a program
  built from older sources.

- Collect the processes and directories of tests and verification steps: a
  started engine stack is stopped also after a failing step, each tool test
  and browser script runs in its own process group that is killed at its end
  and at its limit, and Rust tests remove their temporary directories also
  when an assertion fails.

- Require the CSS checkout in the engine mount check case of
  `tools/test_local_paths.py`, which failed after the mount was added.

- Refuse a full run with untracked files: the guard of `make check` and
  `make rerun-failed` refuses while files that are neither tracked nor ignored
  exist and names each, because a step would read such a file although the
  recorded tree does not hold it. `tools/check.py` reads only tracked files
  and fails for each untracked file that is not ignored.

- Build, lint and test `polyspec-ssr-server` natively on AArch64 Linux in the full suite:
  `make check-linux` runs Clippy, the example builds and the unit, `development`
  and `process` tests of `polyspec-ssr-server` with the Linux gcc toolchain of the
  checkout's container stack, which mounts every source checkout read only.

- Refuse a push while a checklist item is in progress: the tracked pre-push
  hook `.githooks/pre-push` runs `python3 -m tools.push_gate hook`, which
  reads the checklist of the tip commit of each pushed ref and of the working tree and
  names each item in progress with its ref, commit, ID and title. Every make
  invocation sets `core.hooksPath` to `.githooks`; `make hooks` sets and
  checks it, and `tools/check.py` and the guard of `make check` fail while
  it is not set. The workflow job `push-gate` checks the pushed tip commit and
  pull request head on GitHub the same way.

- Build once for the source events that arrive while a build runs: the
  supervisor starts one rebuild for every queued event and exactly one more for
  the events that arrive during it, so one write no longer builds the
  application once per event, and no event is dropped.

- Compile the Linux source watch: the `inotify` watch module no longer shadows
  the `notify` crate, so `polyspec-ssr-server` compiles for Linux again.

- Observe every source change after `Development::start` returns: on macOS the
  source watch registers the watched directories and regular files with
  `kqueue(2)` through a walk that skips the excluded paths, raises the file
  descriptor limit it needs or fails naming the count, the limit and the
  largest directories, and registers new entries before it reports them.
  `start` builds only after the event of a sentinel file in its own watched
  directory, so events of earlier writes no longer start a second build.

- Rebuild when the source watch reports dropped events: a `Rescan` event,
  which the watch sends when the file event service dropped events, starts a
  rebuild and a warning unless all its paths are excluded.

- Run the full suite once after every active checklist item is complete:
  `var/full-run.json` records its result and step times, and a failure becomes
  a new checklist item. During development only the Red and Green tests that
  own a change run.

- Use a private Unix socket for supervised renderer requests: the supervisor
  creates and owns the socket directory before it starts the render process,
  passes the socket path to the render command and removes only its own
  directory after it collects the process exit; readiness no longer resolves a
  service address. The supervisor rejects a socket path longer than a Unix
  socket address before it creates the socket directory, and readiness names a
  declared path that is not a Unix socket and a socket directory open to other
  users as separate errors. The development cases require that every socket
  directory is removed after its process exit and that a dropped response
  cancels the child response.

- Keep only items and headings in the checklist: `tools/check.py` fails for a
  checklist line that is not blank, a heading, an item line or a continuation
  line of an item. The requirements moved to `docs/requirements.md`, and
  `AGENTS.md` states the checklist format.

- Treat the task list states of GitHub as state markers: `tools/check.py` also
  fails for an x or a capital X between brackets in the checklists that is not
  the state of an item line.

- Allow a state marker in the checklist only as the state of an item:
  `tools/check.py` fails for any other bracketed state marker of
  `docs/checklist.md` and `docs/checklist.ko.md` and names its file, line and
  column. The checklists no longer have a legend; `AGENTS.md` defines the
  states, and the texts name states in words.

- Refuse the full suite before any step unless it may run: `make check` runs
  through `tools/full_run.py`, which refuses while a checklist item is `[~]`
  (listing each ID and title), while tracked files have uncommitted changes and
  when `var/full-run.json` records a full run of the same tree. The record is
  written before the first step and after each step, so a killed run stays
  `incomplete`. `make rerun-failed` runs only the targets of the record of the
  current tree that did not pass. A `make` without a target runs `make check`.

- Run the steps of `make check` under the checkout lock `var/locks/check.lock`,
  so a second `make check` of the same checkout no longer reinstalls the
  build-probe fixture while the first run's tests read them; it
  is refused with the holder's checkout, pid and process start time.

- Give each checkout its own engine verification stack: the Compose project,
  container and image are named from the SHA-256 of the checkout path, so a
  verification of another checkout no longer replaces the stack of a running
  one. `make verify-engine-linux-arm64` and `make verify-engine-down` run
  `tools/verify_engine_linux.py` under the checkout lock
  `var/locks/engine-verification.lock`; a second run is refused with the
  holder's checkout, pid and process start time, and each step prints its
  result and elapsed time without a time limit.

- Read the workspace metadata of the ownership check without a time limit,
  reporting it as a step with its elapsed time and judging it by its exit code.

- Give the browser launch of the browser cases no time limit. The launch and
  close are reported with their elapsed time and decided by their result; page
  steps keep their 60 s limit.

- Build the workspace tests once before the ownership cases and run every case
  in one nextest run on that build.

- Measure the render CPU time in the worker thread for the render benchmark,
  so concurrent calls no longer add to the CPU time of a call.

- Prove that one request deadline covers the queue wait and the execution with a
  queue wait and an execution that each fit in the timeout but not together.

- Decide the browser cases by page events and process exit, print each browser
  step with its elapsed time, and bound a browser case only to detect a hang.

- Judge the render benchmark by process CPU time: CPU renders per second and
  CPU p99 have limits, and wall-clock values and context reset times are
  reported only.

- Judge the queued cancellation and timeout cases by a test-only queue entry
  signal and the returned error kind instead of wall-clock limits.

- State that only the owning Red and Green tests and `tools/check.py` run
  before a commit, and that the full suite runs once when every active
  checklist item is complete.

- Release the scheduler of a finished React stream render, so the context of
  every finished render can be collected and repeated renders keep the heap
  bounded.

- Declare each local V8, CSS and JSON checkout path once in the root manifest.
  The engine Compose preparation and mount check read those paths, and a
  tracked case rejects an absent or differing path in any manifest.

- Resolve configured JavaScript imports with the standard resolver so scoped
  conditional exports select their declared entry and unexported subpaths fail.
  Preserve the single dependency directory and reject nested dependency replacements.

- Resolve every application package import from one configured dependency directory: bare
  package specifiers resolve only there, so a nested `node_modules` copy of the same package
  cannot create a second module instance beside the render context provider and consumer.

- Verify the official Linux V8 archive and binding inside the engine verification image: digest
  checks, read-only source mounts including ordered-json, and native same-snapshot parallel
  rendering with no V8 source compilation.

- Cancel closed and expired streams, bound native chunk transmission, and acknowledge cleanup before
  worker reuse. Require explicit pool byte and heap limits. Report unavailable workers through a
  closure event and return errors from thread cleanup.

- Use one immutable application snapshot per renderer process with the official local V8 archive
  and matching binding. Public snapshot creation completes before concurrent workers restore the
  same blob. Different bundle keys require separate renderer processes because one process retains
  one initialized application. Verification rejects missing inputs, changed hashes and mismatched
  features. Native parallel restoration, React streams, separate application processes and existing
  performance limits are checked with the official archive.
- Reject Svelte compiler entry during snapshot initialization and after a successful snapshot.
  A shared process contract protects V8 entry through isolate disposal; completed compilation may
  precede rendering, and ordinary Rust bundling remains available. Svelte transformation preserves
  compiler error causes. Test ownership requires actual behavior use and its declared consumer test.

- Run build and render commands in separate processes, validate completed build directories and
  actual SSR preparation before serving requests, retain active response processes during replacement,
  and restart an exited render process from its completed build. Report rebuild and shutdown failures,
  collect child exit statuses, and watch declared source inputs with explicit output exclusions.

- Add `Build::write` and `Build::read` for complete build directories named by the manifest
  SHA-256. Publish private and public files with one directory rename, preserve equal writes and
  previous builds, and reject changed manifests, missing or extra files, invalid paths, symbolic
  links and incorrect file digests. Report publication and cleanup failures together.

- Require a target-specific Linux V8 archive with independent isolate-group support for development image rendering.

- Resolve application packages from the configured dependency directory so nested package copies do not create separate module instances.

- Complete the Vue, Svelte and vanilla adapters. `features.json` names an exact executable case for
  every supported feature, and `make check` runs every declared case.
- Record supported adapter rendering, public asset, source map and React form capabilities in
  `features.json`. Each declared capability names one exact executable case. The feature checker
  runs every case, rejects missing evidence or duplicate feature declarations and fails on a missing test, nonzero
  command exit or timeout because a declaration or source inspection cannot establish support.
- Enforce test ownership for shared page and build behavior. Each owner and every direct
  consuming crate declares a specific integration test function, and `make check` executes
  each case. Missing, misplaced, empty, failed, ignored and timed-out tests fail because a
  file name alone does not prove behavior in the crate that uses it.
- Verify React streaming across HTTP shell status, late body errors, request nonces, timer
  isolation, render metrics, and browser recovery after a Suspense error.
- Verify that a React stream reader rejection after a body chunk returns that chunk and an
  explicit body error while the initial HTTP status and headers remain fixed.
- Correct the React adapter and runtime documents to name the framework and application entries,
  `Pool::new_react`, `Pool::render_stream`, and `ReactAdapter::stream_parts`. A record check rejects
  the obsolete React pool call because it describes a different render path.
- Stream React SSR documents after the shell is ready. Restore the framework and application in
  one request context, keep scheduling outside the application global, and preserve React's
  Suspense fallback and client recovery instructions. Generate a distinct operating-system nonce
  per request, apply it to inline scripts and styles across output chunks, and record React errors
  with that nonce. Reject React server chunks because its application bundle is an IIFE.
- Verify the React bundle execution boundary in an executable V8 fixture. A top-level React
  class and context use the same React instance while framework scheduling remains in a
  function argument and the application global has no timer.
- Remove procedure-only S-0-2-1 from the checklist and keep the priority and test placement
  rules in AGENTS.md. Checklist items require a repository artifact and verifiable completion
  evidence.
- Add Svelte source compilation inside the Rust build process and the Svelte adapter. The build
  publishes component CSS with a content-hashed URL; the adapter places server head output in the
  document. The HTTP server serves the ESM build and component CSS, applies a request nonce to
  inline head and body content, and maps JavaScript failures through the build source maps.
  Browser cases verify hydration, client events, head output, component style, CSR and static
  shells because generated bundles alone do not establish browser behavior.
- Select React, Vue, Svelte and vanilla client hydration from the render mode recorded in each document.
  An empty SSR body still selects hydration, while CSR selects a new render. Missing or invalid
  modes fail because body content does not identify the requested render operation.
- Verify a React form with repeated rows of the tracked fixture `FormRowsApp.tsx` with two
  server renders. The case checks nested `rows.<key>.<field>` names, row keys of `row-` and eight
  hexadecimal digits and a new key on the next render because each
  request must create independent row identities.

- Require a `head` string in every runtime render result and preserve its bytes. Missing or
  invalid values fail because render output must remain complete. React, Vue and vanilla return
  an empty head and reject nonempty head output without a document placement rule.

- Add the Vue adapter with Promise-based server rendering and browser hydration. The browser
  verifies the original server DOM node and an attached event handler because a generated bundle
  does not establish hydration behavior.

- Add the vanilla adapter with server and client entries, SSR output state, CSR documents and
  static shells. A browser verifies server DOM identity and client event handling because generated
  source alone cannot establish hydration behavior.

- Add the development procedures (`AGENTS.md`), the checklist and this changelog.
- Synchronize the combined pool wait and execution timeout case on queue admission because a
  thread start signal does not establish which render owns the worker.
- Add the render, build and engine requirements of ssr and the build and engine checks to the checklist.
- Create the Rust 1.98.1 workspace with eight crates and the check and bench commands. The check
  command validates records, terminology, documents, dependencies and unit tests because these
  requirements apply to every crate.
- Exclude installed package documents from record and terminology checks because packages are
  dependencies outside the maintained source.
- Verify the engine dependency versions and V8 archive digests. Full builds link for macOS arm64,
  macOS x86_64, Linux x86_64 and Linux AArch64. The Linux AArch64 program executes in a native
  container.
- Build with a verified local V8 archive and Cargo network access disabled. Missing or changed
  archives and failed downloads return errors.
- Bind the engine verification Cargo cache and build output to ignored host directories. The
  source remains read only, and a replacement engine container reuses the linked program.
- Select the maintained compile-time identifier macro package in the local V8 source checkout.
  Rebuild and link the engine on four targets, mount that checkout read only in the test container,
  and retain the cached build across container replacement because the previous macro package has
  no maintained release.
- Allow the specified dependency licenses in cargo-deny because build dependencies use licenses
  beyond MIT. Unknown licenses remain errors.
- Allow BSL-1.0 for xxhash-rust 0.8.18 and Apache-2.0 with the LLVM exception for
  dragonbox_ecma 0.1.12 because the build depends on these exact crate versions.
- Verify React TSX server and client bundles, CSS imports and hashed asset output with Rust APIs.
  Package CSS resolution and URL replacement require explicit source and asset handling.
- Define the page JSON contract and render result in `polyspec-ssr-core`. Ordered JSON preserves object
  order and number tokens; invalid fields and input return errors.
- Exclude unpublished local packages from the license check because the JSON package is consumed
  from a local source checkout. Registry dependencies remain subject to the license policy.
- Reject repeated decoded object keys in page JSON at every depth because retaining only the
  last value discards request input.
- Build React TSX server and client bundles, CSS and hashed assets in `polyspec-ssr-build`. The build
  returns all files and an ordered-json manifest; the asset URL hook gives both bundles the public
  absolute URL because a chunk-relative URL does not identify the file from a page document.
- Execute server bundles in a bounded `polyspec-ssr-runtime` worker pool. Each worker owns an isolate,
  resets global state for each request, terminates scripts at the timeout,
  and provides console output and operating system random values. Invalid results and unavailable
  Web APIs return explicit errors because output must not be omitted or changed silently.
- Initialize server globals in a V8 snapshot keyed by the bundle hash and pinned versions.
  Restore each request context from the snapshot because rebuilding server globals on each
  render repeats initialization. Changed keys create snapshots; unused snapshot bytes are released.
  Bundle initialization returns an error at the configured timeout.
- Create an independent V8 isolate group for each server snapshot and restore its worker isolates
  in that group. The release source build provides separate pointer cages and external code space so
  distinct application snapshots restore concurrently; unavailable group support fails.
- Verify a separate local V8 source archive and its generated binding by SHA-256 before routine
  checks and benchmarks link them.
  Direct source verification remains explicit because repeated GN generation can rebuild the archive.
- Evaluate private server entries and chunks as ECMAScript modules. Generated entries export
  `render` without a global property; relative imports resolve only to supplied server files.
  Restore module data per request context and map each file's JavaScript stack through its own
  source map. Missing files and unfinished evaluation fail because the render code is incomplete.
- Provide UTF-8 `TextEncoder` in every render context because the React server bundle requires it.
  Encoding and bounded `encodeInto` handle non-ASCII text and unpaired surrogates; invalid calls
  fail without adding file, network or timer operations.
- Create the React adapter entries and document renderer. SSR returns React HTML and output state;
  CSR returns an empty root with unchanged input state, and a static shell can serve multiple paths.
  Escaped JSON prevents a request from closing its script element. A browser verifies hydration,
  CSR rendering and shell reuse because bundle inspection cannot verify DOM behavior.
- Publish verified public build files into an absolute directory. Publication preserves equal
  files and rejects changed files because published URLs must keep their bytes. Serve exact public
  URLs with their content types and digests; server output remains private.
- Record private deterministic server source maps in the build manifest. The server verifies the
  map and server bundle, renders `POST /_render`, and returns explicit HTTP failures. It records
  render time, pool wait and V8 heap use through tracing. JavaScript failures report source-mapped
  stack locations because generated bundle positions do not identify application source lines.
- Add a development server that rebuilds bundles after file events and replaces its server and
  render pool together. Invalid changes make requests fail with the build cause until a valid
  change restores service because serving older output would hide the failed build.
- Measure a fixed SSR page with concurrent calls in `make bench`. The command reports throughput,
  call latency, context creation time and V8 heap change, and fails when a measured limit is exceeded.
- Complete V8 microtasks before reading a Promise render result. Fulfilled results use the normal
  result contract; rejection returns its message and stack, and a result with no scheduled
  completion fails. A separate-context React test verifies that an application component renders
  without receiving a framework timer because framework scheduling must remain private.
- Build a separate private React framework bundle and application server bundle with one shared
  React instance because independent copies reject application hooks. The build validates the
  framework input, records its digest and source map, and keeps it outside public files. A test
  renders a component using `useId` in a context without the framework timer.
- Give the React entry build and browser hydration tests separate directories for generated
  server, framework and client files. A concurrent test verifies that each reads its own files
  because a shared path can replace build inputs during parallel checks.
- Handle React CSS imports as explicit JavaScript-free modules and preserve data URLs during stylesheet bundling.
