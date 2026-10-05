# Development procedures

The user's instructions take precedence. [Korean](AGENTS.ko.md).

## Repository

- The remote `origin` is `github.com/polyspec/ssr`. A push happens only when no checklist item,
  sub-items included, is `[~]`, neither in a pushed commit nor in the working tree. The tracked
  pre-push hook `.githooks/pre-push` runs `python3 -m tools.push_gate hook`, which refuses such a
  push and names each item with its ID and title. Every make invocation sets `core.hooksPath` to
  `.githooks` when it differs; `make hooks` sets and checks it, and `tools/check.py` and the guard
  of `make check` fail while it is not set. The workflow `.github/workflows/push-gate.yml` runs
  `python3 -m tools.push_gate commit` in the job `push-gate` on every push and pull request and
  fails the same way, also for a push that did not run the hook.
- Name branches `{type}/{shortname}-{checklist ID}` and worktrees
  `{project}-{shortname}-{checklist ID}`. After integrating a branch into `main`, verify its commits
  or equivalent changes are present and its worktree is clean. Before removal, preserve any files
  excluded by `.gitignore` that exist only in that worktree and are still needed. Then remove the
  worktree and local branch immediately.
  Preserve unintegrated or active work.
- Before committing the related feature, cherry-pick useful commits from a test-only branch that
  cannot be integrated into `main`, discard the remaining test-only changes, and remove its worktree
  and branch. If removal is impossible, first add a numbered sub-item to the owning checklist with
  the cause and exact removal condition.
- JSON documents of the public contract use ordered-json.
- Develop one `0.0.1` library. The crates are `ssr-core`, `ssr-build`, `ssr-runtime`,
  `ssr-adapter-react`, `ssr-adapter-vue`, `ssr-adapter-svelte`, `ssr-adapter-vanilla` and `ssr-server`.
- The library runs no Node process. Production build commands and render servers run in separate Rust processes.
  A renderer process selects one immutable bundle and snapshot key for its lifetime. New bundles
  use new renderer processes; request concurrency and services share the selected full application bundle.
- Preserve generated JavaScript, component CSS and server head in their defined destinations.
  Missing required output and compiler warnings fail the build or render. Publish only source maps
  that identify the final output accurately.
- The test fixtures are tracked in this repository. Package tests install the npm packages of the
  tracked fixture lock with `npm ci` without symbolic links.

## Vocabulary and design

- Do not create new words for the public API or documents beyond the names in the checklist. When a
  new name is needed, collect every needed name and ask the user once for approval.
- Keep no backward compatibility, fallback or hidden error; invalid input is an error.
- A snapshot key includes the SHA-256 of server entry and chunk paths and bytes, the pinned
  deno_core version and the library version. Restored contexts use their own global objects for
  native callbacks. Consume every V8 snapshot creator on success and initialization failure, apply
  the configured timeout to snapshot initialization, and retain the one snapshot until process exit.
  Dispose its creator before restoring concurrent worker isolates through the public V8 API.
  A different bundle key is an explicit error even after all pools close.
- Protect Svelte compiler V8 entry and snapshot initialization with the common process contract.
  Compiler operations may overlap each other but cannot overlap snapshot creation or follow a
  successful snapshot. Dispose the compiler runtime and snapshot creator before releasing their
  process access. A failed snapshot must dispose every isolate before another engine entry.
  Ordinary Rust bundling and completed compilation before rendering remain supported.
- A managed renderer child follows its stdin shutdown contract. The parent retains its stdin handle
  while that generation accepts or drains requests, closes it for shutdown, then awaits and records
  the child's exit. Manage stdin ownership separately from exit observation. A readiness task must
  not accidentally close the handle. A child that exceeds its graceful shutdown duration is killed
  and waited for, and shutdown reports that failure.
- An ordinary server entry exports `render` as an ECMAScript module. Compile its entry and every
  private server chunk into the snapshot once, resolve relative imports against the exact supplied
  files, and restore independent module data in each request context. Keep the render export out of
  the global object. A missing module, invalid import or unfinished evaluation is an error.
- Evaluate React framework and application bundles in one snapshot context so application module
  initialization and rendering use one React instance. Give framework scheduling a lexical binding
  and remove it from the global before evaluating application code. Do not evaluate either bundle
  on each request.
- Require explicit pool limits and request cancellation. Return worker capacity only after context
  cleanup, report a failed cleanup, and replace the renderer process when native work cannot stop.
- Public API documents name the current entry, pool and render methods. Check
  the English and Korean contracts against executable use paths; remove descriptions of replaced
  calls when the API changes.
- When a decision of the agent is shown wrong, the report names that decision first.
- `var/handoff/work.md` lists the specification sources of each item; it is not tracked.

## Checklist

- `docs/checklist.md` is the only task tracker. `docs/checklist.ko.md` has identical task IDs and
  states. Do not create another checklist. Split an item into numbered sub-items or add items when
  needed.
- Record repository-specific work with a concrete artifact and verifiable completion evidence in
  the checklist. Keep standing instructions for how to work in this file. A requested policy change
  updates this file; it becomes a checklist item only when it also requires a separate repository
  implementation or check. Classify each new request before adding an item.
- States: `[ ]` waiting, `[~]` in progress, `[o]` complete, `[!]` temporarily bypassed.
  Use `[!]` only when an unfinished item must be deliberately bypassed because work otherwise
  cannot advance to the next checklist item. Do not use it to defer a difficult item while work
  remains possible. A bypass is not completion. Record the cause and retry condition in the item,
  and resume it without waiting for permission when the retry condition is met. Audit only `[!]`
  items and their causes and retry conditions; do not repeat unrelated full test suites for that
  audit.
- The checklists hold only items and headings. An item is an item line, which starts with a hyphen,
  a space, its state, a space and its ID, and the lines indented by two spaces that directly follow
  it. Each item names its dependencies and its completion evidence. Requirements are in
  `docs/requirements.md`; rules are in this file. A state marker, also an x or a capital X between
  brackets as Markdown task lists write it, appears only as the state of an item line; the
  checklists have no legend, and their texts name states in words. `tools/check.py` fails for any
  other line or marker and names its file, line and column.
- Work in parallel and control work in progress. Independent items A, B, C and D may run at the same
  time; starting new work is not restricted. Do not advance existing items partially while adding
  more `[~]` items so that only the number of unfinished items grows. For example, A[~] and D[~]
  together are allowed; starting D after part of A, then E, F and G after part of D, is not. Prefer
  completing items in progress over starting new ones, so that `[o]` grows continuously.
- For repository work or a newly found defect, record its priority, acceptance criteria, and
  dependencies in this checklist and update the specification before implementation. Finish current work unless
  the user explicitly requires immediate action or the defect must be fixed first. Run independent
  items in parallel without adding another task list. An uncommitted worktree covers one item;
  commit that item's implementation, tests, and records when marking it `[o]`.
- Marking an item `[o]` writes its implementation, tests, records and changelog entry in one
  commit. A received instruction is triaged first: finish the item in progress unless the
  instruction is explicit and urgent, then place the new work by priority before starting it.
- Write commit messages in English as `type(scope): subject (#issue)`: a subject of at most 50
  characters, capitalized, imperative, without a trailing period; a blank line; a body wrapped
  near 72 characters explaining what changed and why; an optional footer for references. The
  type is one of feat, fix, docs, style, refactor, test or chore.
- During development run only the Red and Green tests that own the change; run the full suite
  once, when every active checklist item is complete, never after each fix or item. Every test
  reports its own running, completion, success or failure with its elapsed time and has its own
  timeout; a whole-suite timeout is not used. A long operation gets detailed step logs instead
  of a timeout, so its process and result stay observable.
- `make check` enforces this: before any step it refuses while a checklist item, sub-items
  included, is `[~]`, while tracked files have uncommitted changes, while files that are neither
  tracked nor ignored exist, while the pre-push hook is not installed, and when `var/full-run.json` records a full run of the same tree. Commit, complete every active item, then run `make check`
  once. `make rerun-failed` reruns, on the recorded tree, only the targets that did not pass, for
  a failure whose cause lies outside the tree (an environment or a machine resource). A failure
  of the code is fixed as a checklist item; its commit makes a new tree, whose full suite runs
  once when every active item is complete. Do not delete or edit the record to run again.
- A newly found issue in a completed item is added as a numbered sub-item; the completed item does
  not go back to `[~]`.
- Marking an item `[o]` requires its implementation, tests and records to be committed in the same
  step. Uncommitted changes cover one item only.

## Implementation

1. Define or update the specification and acceptance criteria before implementation. Do not weaken
   a correct criterion to pass a test.
2. For a defect, reproduce the failure with a tracked RED test. For a plausible failure not yet
   observed, first write a deterministic RED case for the input and required result that would
   expose it. Confirm the test fails for the intended reason before changing the implementation;
   then correct the cause and run the same case and relevant use-path tests to GREEN. If a case
   cannot expose the problem, investigate it instead of weakening the criterion.
   Put a behavior's base tests in its owning crate and test its actual use in each consuming crate.
   Both tests execute; the presence of a test file alone is not completion evidence.
3. Choose the simplest complete implementation. No compatibility layers, fallbacks, data
   conversion or abstractions for unspecified requirements. Invalid input is an error, never a
   plausible default.
4. `ssr-core` depends on no other crate of this repository; adapters depend only on `ssr-core` and
   `ssr-runtime`.
5. Dependencies are pinned to exact versions in the workspace manifest and verified against
   official release information. `Cargo.lock` is committed.
6. A missing test environment is a failure, never a skip. Each test has its own timeout and reports
   start, pass, fail, skip and timeout with elapsed time. Check exit codes; do not hide them behind
   pipes. Check at least 10 GiB free disk space and the pinned local V8 inputs before a long run.
   `make check` and `make bench` verify the target, SHA-256, binding and Cargo features of the
   official local archive before linking. Missing or changed inputs fail; do not download V8 or
   substitute a V8 source build. Long builds report their commands and progress without a total
   duration limit; test cases retain individual time limits.
7. Performance claims are measured by the maintained benchmark (`make bench`) with recorded limits.
8. No polling where an event exists, no symbolic links, no relative paths in configuration and no
   temporary scripts for repeatable work: repeatable commands are make targets or tools in Git.
9. Containerctl verification mounts source checkouts read only. When a build needs host inspection
   and cache persistence across container replacement, mount writable caches and build output from
   absolute host paths under ignored `var/`; verify mount access and cache reuse after replacement.
10. A client entry selects hydration or a new render from the explicit page render mode in the
    document, including when SSR HTML is empty. Missing or invalid modes fail. A hydration
    acceptance test runs in a browser and verifies that the server DOM node remains the same node
    after client hydration. Bundle text inspection does not prove hydration.
11. Concurrent tests that generate application entry files use separate directories for each test.

## Records

- Records are the commit log, documentation, comments, i18n text and `docs/changelog.md`
  (+ `.ko.md`). They describe the current product and its changes with explicit subjects, actions
  and objects, in plain wording without metaphors, personification or colloquial words. Operation
  names are used directly (create, publish, receive, register, remove, return, fail). A cause is
  stated in one sentence. Words such as envelope, gate, orphan, adopt, retire, dead, first-class,
  carries, speaks and answers are not used for product behavior.
- A record states a defect as a fact about ssr itself: the input, the behaviour and the expected
  behaviour. It names no reporter, no source and nothing outside ssr, and holds no origin of the
  code, no porting or migration history, no authorship or tool attribution (including Co-Authored-By
  trailers) and no conversation or investigation history; a sentence without such a fact about ssr
  is deleted. Such context stays in local memory outside Git.
- English is canonical. Update the `.ko.md` file in the same change with equal information.
- Before each commit, run the Red and Green tests that own the change and `tools/check.py`,
  which runs the record and terminology checks on tracked files and fails for each file that is
  neither tracked nor ignored.
