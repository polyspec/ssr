# Development procedures

The user's instructions take precedence. [Korean](AGENTS.ko.md).

## Repository

- This repository is developed locally. Do not add remotes and do not push.
- JSON documents of the public contract use ordered-json.
- Develop one `0.0.1` library. The crates are `ssr-core`, `ssr-build`, `ssr-runtime`,
  `ssr-adapter-react`, `ssr-adapter-vue`, `ssr-adapter-svelte`, `ssr-adapter-vanilla` and `ssr-server`.
- The library runs no Node process: bundling and rendering run inside the Rust process.

## Checklist

- `docs/checklist.md` is the only task tracker. `docs/checklist.ko.md` has identical task IDs and
  states. Do not create another checklist. Split an item into numbered sub-items or add items when
  needed.
- States: `[ ]` waiting, `[~]` in progress, `[o]` complete.
- Work in parallel and control work in progress. Independent items A, B, C and D may run at the same
  time; starting new work is not restricted. Do not advance existing items partially while adding
  more `[~]` items so that only the number of unfinished items grows. For example, A[~] and D[~]
  together are allowed; starting D after part of A, then E, F and G after part of D, is not. Prefer
  completing items in progress over starting new ones, so that `[o]` grows continuously.
- A newly found issue in a completed item is added as a numbered sub-item; the completed item does
  not go back to `[~]`.
- Marking an item `[o]` requires its implementation, tests and records to be committed in the same
  step. Uncommitted changes cover one item only.

## Implementation

1. Define or update the specification and acceptance criteria before implementation. Do not weaken
   a correct criterion to pass a test.
2. Reproduce a defect or missing behavior with a tracked test, then correct the cause.
3. Choose the simplest complete implementation. No compatibility layers, fallbacks, data
   conversion or abstractions for unspecified requirements. Invalid input is an error, never a
   plausible default.
4. `ssr-core` depends on no other crate of this repository; adapters depend only on `ssr-core` and
   `ssr-runtime`.
5. Dependencies are pinned to exact versions in the workspace manifest and verified against
   official release information. `Cargo.lock` is committed.
6. A missing test environment is a failure, never a skip. Each test has its own timeout and reports
   start, pass, fail, skip and timeout with elapsed time. Check exit codes; do not hide them behind
   pipes. Check free disk space and the pinned V8 archive before a long run.
7. Performance claims are measured by the maintained benchmark (`make bench`) with recorded limits.
8. No polling where an event exists, no symbolic links, no relative paths in configuration and no
   temporary scripts for repeatable work: repeatable commands are make targets or tools in Git.

## Records

- Records are the commit log, documentation, comments, i18n text and `docs/changelog.md`
  (+ `.ko.md`). They describe the current product and its changes with explicit subjects, actions
  and objects, in plain wording without metaphors.
- A record states a defect as a fact about ssr itself: the input, the behaviour and the expected
  behaviour. It names no reporter, no source and nothing outside ssr, and holds no origin of the
  code, no porting or migration history, no authorship or tool attribution (including Co-Authored-By
  trailers) and no conversation or investigation history; a sentence without such a fact about ssr
  is deleted. Such context stays in local memory outside Git.
- English is canonical. Update the `.ko.md` file in the same change with equal information.
- Run `make check`, which includes the record and terminology checks, before each commit.
