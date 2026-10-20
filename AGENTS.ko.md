<!-- doc-id: agents -->
<!-- source-sha256: ae3d8e72c34d6f25649c217924f958722c610f88441f3787776aca772fbdaf95 -->
# 개발 절차

[English](AGENTS.md).

모든 polyspec 저장소의 작업 원칙과 모든 저장소가 따르는 작업 규칙은 [PRINCIPLES.ko.md](PRINCIPLES.ko.md)에 있다. 이 파일은 ssr의 규칙만 적는다.

## 공유 도구

- Own: ssr은 package `@polyspec/kit`(kit)의 공유 도구를 실행한다. `package.json`은 이를 release asset의 URL과 integrity로 고정하고 `package-lock.json`이 이를 담는다. checkout은 이를 두 번의 online 단계로 설치한다. `make install-tools TOOLS=npm`은 `packageManager`의 npm을 `var/tools`에 설치하고, `make install-packages`는 그 npm으로 `package-lock.json`을 설치한다. Makefile은 `scripts/kit/kit-bootstrap.mk`를 include하고, 이는 설치된 package의 `kit.mk`를 include한다. 설치 전에는 그 두 target만 실행된다.
  Enforced by: `make kit-check`
- Own: kit은 `make kit-sync`(그 `package.json`의 `kit.copies`)로 자기 파일의 복사본을 쓴다. 복사본은 `scripts/kit/`, `.github/workflows/release.yml`, `.github/workflows/gates.yml`, `.claude/settings.json`, `PRINCIPLES.md(.ko)`, `config/layout.kit.yml`, lint 설정과 schema다. 복사본은 kit에서 바꾸고 다시 복사하며 여기서 고치지 않는다. 더 새로운 kit은 `package.json`과 `package-lock.json`의 `@polyspec/kit`을 갱신한 뒤 `make install-packages`와 `make kit-sync`로 받는다.
  Enforced by: `make kit-check`
- Own: kit은 Node.js 26.12.0(`mise.toml`의 `node`, `engines.node` `>=26.12.0`)과 Rust 1.99.0(`rust-toolchain.toml`, 모든 Cargo package의 `rust-version`)을 고정한다. ssr은 PHP, Python, Go를 선언하지 않는다.
  Enforced by: `make toolchain-check`
- Own: 한국어 문서는 영어 문서의 sha256인 `<!-- source-sha256: ... -->`를 담고, `make documents-stamp`가 번역이 같은 정보를 담게 된 뒤 이를 쓴다.
  Enforced by: `make docs-check`

## 저장소

- Own: remote `origin`은 `github.com/polyspec/ssr`다. push는 checklist 항목이, sub-item을 포함해, push되는 ref의 tip commit에도 working tree에도 `[~]`가 아닐 때만 한다. 추적되는 hook `.githooks/pre-push`는 `make hooks`가 쓰며 kit의 push check를 실행해 그런 항목을 ID와 제목으로 지목한다. workflow `.github/workflows/push-gate.yml`은 모든 push, pull request, merge group에서 job `push-gate`에 `make ci-push`를 실행하므로, hook을 실행하지 않은 push도 같은 방식으로 실패한다.
  Enforced by: `.githooks/pre-push`, `make push-gate-commit`
- Own: `make ci-push`는 또한 `make check-records`(`scripts/check-records.mts`의 record 검사: 단어, React 문서, 경로, 시간 assertion)와 `make check-commit-records`(push되는 commit인 `RANGE`의 commit message의 단어, 이를 요구한다)를 실행하며, job은 `scripts/`와 `tests/`의 TypeScript를 위해 `make typecheck`와 `make lint-ts`를 실행한다.
  Enforced by: `make check-records`, `make check-commit-records`, `make typecheck`, `make lint-ts`
- Own: 모든 변경은 pull request와 merge queue를 거쳐 `main`에 도달하며, 이 저장소의 어떤 명령도 `main`을 push하지 않는다. `.github/ruleset.json`에 선언한 GitHub ruleset `main`은 pull request(승인 불요), merge method `REBASE`의 merge queue, linear history, 정확히 `push-gate`와 `ci-passed` check를 요구하고, `main`의 force-push와 삭제를 거부하며, bypass actor가 없다. `make github-ruleset`은 ruleset과 선언한 저장소 설정을 만들거나 갱신한다([workspace](docs/operations/workspace.ko.md)). branch는 `git push origin HEAD:refs/heads/<branch>`, `gh pr create --base main --head <branch> --fill`, `gh pr merge <branch> --auto --rebase`로 올린다. rebase는 병합된 commit에 새 hash를 주므로 `git pull --rebase`는 queue가 병합한 로컬 commit을 버린다.
  Enforced by: none, the live settings of GitHub are read with an administrator token that no CI job holds; `make github-ruleset-check` compares them on request.
- Own: 공개 계약의 JSON 문서는 ordered-json을 쓴다.
  Enforced by: none, which JSON documents belong to the public contract is a judgement.
- Own: `0.0.2` library 하나를 개발한다. crate는 `polyspec-ssr-core`, `polyspec-ssr-build`, `polyspec-ssr-runtime`, `polyspec-ssr-adapter-react`, `polyspec-ssr-adapter-vue`, `polyspec-ssr-adapter-svelte`, `polyspec-ssr-adapter-vanilla`, `polyspec-ssr-adapter-support`(adapter가 공유하는 code로 다른 crate는 쓰지 않는다), `polyspec-ssr-nonce`, `polyspec-ssr-server`이고, 각각 접두사 `polyspec-ssr-` 없이 `packages/<name>`에 있으며(`packages/core` 등) library는 `polyspec_ssr_core` 등이다.
  Enforced by: `make layout-check`, `make check-nextest`
- Own: library는 Node process를 실행하지 않는다. production build 명령과 render server는 별도의 Rust process에서 실행된다. renderer process는 자기 수명 동안 불변의 bundle 하나와 snapshot key 하나를 고른다. 새 bundle은 새 renderer process를 쓰며, 요청 동시성과 서비스는 선택된 전체 application bundle을 공유한다.
  Enforced by: `make check-nextest`
- Own: 생성된 JavaScript, component CSS, server head는 정해진 위치에 보존한다. 필요한 출력이 없거나 compiler warning이 있으면 build나 render가 실패한다. 최종 출력을 정확히 가리키는 source map만 게시한다.
  Enforced by: `make check-nextest`
- Own: test fixture는 이 저장소에 추적된다. package test는 추적되는 fixture lock의 npm package를 symbolic link 없이 `npm ci`로, lock digest마다 한 번 설치하며, `make check-fixture-filter`는 setup script의 filter가 그 설치를 읽는 test binary를 정확히 지목하는지 검사한다.
  Enforced by: `make check-fixture-filter`, `make check-nextest`
- Own: 생성된 파일은 Git에 추적하지 않는다. dependency 설치(`node_modules/`, `vendor/`, 가상 환경), build 출력(`dist/`, `target/`, `*.tsbuildinfo`), cache(`.cache/`, `__pycache__/`, `.pytest_cache/`), test 실행 산출물(`test-results/`, `playwright-report/`, PHPUnit cache), 도구 download(`var/`)가 그것이다. 이런 종류에서 추적되는 파일은 lock 파일(`package-lock.json`, `composer.lock`, `Cargo.lock`, uv lock, `mise.lock`), 이 파일이 지목하는 commit된 생성 문서, test fixture뿐이며, `config/generated.json`이 예외를 선언하고 각 `.gitignore`가 이 경로를 덮는다. 새 generator는 같은 commit에서 ignore 항목을 추가한다.
  Enforced by: `.githooks/pre-commit`

## 어휘와 설계

- Own: public API나 문서에 checklist에 있는 이름 밖의 새 단어를 만들지 않는다. 새 이름이 필요하면 필요한 모든 이름을 모아 사용자에게 한 번 승인을 묻는다.
  Enforced by: none, whether a name is new is a judgement; `make check-records` rejects the unapproved terms.
- Own: snapshot key는 server entry와 chunk 경로 및 byte의 SHA-256, 고정된 deno_core version, library version을 포함한다. 복원된 context는 native callback에 자기 global object를 쓴다. 모든 V8 snapshot creator는 성공과 초기화 실패 때 소비하고, 설정된 timeout을 snapshot 초기화에 적용하며, 그 snapshot 하나를 process 종료까지 유지한다. 동시 worker isolate를 public V8 API로 복원하기 전에 creator를 dispose한다. 다른 bundle key는 모든 pool이 닫힌 뒤에도 명시적 오류다.
  Enforced by: `make check-nextest`
- Own: Svelte compiler V8 entry와 snapshot 초기화는 공통 process 계약으로 보호한다. compiler 작업은 서로 겹칠 수 있으나 snapshot 생성과 겹치거나 성공한 snapshot 뒤에 올 수 없다. compiler runtime과 snapshot creator는 process 접근을 놓기 전에 dispose한다. 실패한 snapshot은 다른 engine 진입 전에 모든 isolate를 dispose한다. 일반 Rust bundling과 render 전에 끝난 compile은 계속 지원한다.
  Enforced by: `make check-nextest`
- Own: 관리되는 renderer child는 stdin 종료 계약을 따른다. parent는 그 generation이 요청을 받거나 비우는 동안 stdin handle을 유지하고, 종료할 때 닫은 뒤 child의 종료를 기다리고 기록한다. stdin 소유권은 종료 관찰과 따로 관리한다. readiness task가 handle을 실수로 닫아서는 안 된다. graceful 종료 시간을 넘긴 child는 kill하고 기다리며, 종료는 그 실패를 보고한다.
  Enforced by: `make check-nextest`
- Own: 일반 server entry는 ECMAScript module로 `render`를 export한다. entry와 모든 private server chunk를 snapshot에 한 번 compile하고, 상대 import는 정확히 공급된 파일에 대해 해석하며, 독립된 module data를 요청 context마다 복원한다. render export는 global object 밖에 둔다. 없는 module, 잘못된 import, 끝나지 않은 평가는 오류다.
  Enforced by: `make check-nextest`
- Own: React framework와 application bundle은 하나의 snapshot context에서 평가해 application module 초기화와 render가 React instance 하나를 쓰게 한다. framework scheduling은 lexical binding을 갖고 application code를 평가하기 전에 global에서 제거한다. 어느 bundle도 요청마다 평가하지 않는다.
  Enforced by: `make check-nextest`
- Own: pool 한도와 요청 취소는 명시적이다. worker 용량은 context 정리 뒤에만 반환하고, 정리 실패는 보고하며, native 작업을 멈출 수 없으면 renderer process를 교체한다.
  Enforced by: `make check-nextest`
- Own: client entry는 SSR HTML이 비어 있을 때도 문서의 명시적 page render mode로 hydration이나 새 render를 고른다. 없거나 잘못된 mode는 실패한다. hydration acceptance test는 browser에서 실행되어 client hydration 뒤에도 server DOM node가 같은 node로 남는지 검증하며, bundle 텍스트 검사는 hydration을 증명하지 않는다.
  Enforced by: `make check-nextest`
- Own: public API 문서는 현재의 entry, pool, render method를 지목한다. 영어와 한국어 계약은 실행 가능한 사용 경로와 대조해 검사하고, API가 바뀌면 교체된 호출의 설명을 제거한다.
  Enforced by: `make check-records`, `make docs-check`
- Own: agent의 결정이 틀렸음이 밝혀지면 보고는 그 결정을 먼저 지목한다. `var/handoff/work.md`는 각 항목의 명세 출처를 나열하며 추적하지 않는다.
  Enforced by: none, which decision was shown wrong is a judgement.

## 구현

- Own: 동작의 기본 test는 그것을 소유한 crate에, 실제 사용 test는 그것을 쓰는 각 crate에 둔다. 두 test가 모두 실행되며, test 파일이 있다는 것만으로는 증거가 아니다. `config/test-ownership.json`이 공유 동작을 각각 선언한다([test ownership](docs/operations/test-ownership.ko.md)).
  Enforced by: `make check-ownership`
- Own: `polyspec-ssr-core`는 이 저장소의 다른 crate에 의존하지 않고, adapter는 `polyspec-ssr-core`, `polyspec-ssr-runtime`, `polyspec-ssr-adapter-support`에만 의존한다.
  Enforced by: `make check-deny`, `make check-ownership`
- Own: dependency는 workspace manifest에서 정확한 version으로 고정하고 공식 release 정보로 검증한다. `Cargo.lock`은 commit한다.
  Enforced by: `make dependency-policy-check`, `make check-deny`
- Own: 없는 test 환경은 실패이며 skip이 아니다. 각 test는 자기 timeout을 갖고 시작, 통과, 실패, skip, timeout을 걸린 시간과 함께 보고한다. 긴 실행 전에 10 GiB 이상의 여유 disk와 고정된 로컬 V8 입력을 확인한다. `make verify-archive`는 link 전에 공식 로컬 archive의 target, SHA-256, binding, Cargo feature를 검증하며, 어떤 build도 V8을 download하거나 V8 source build로 대체하지 않는다. CI runner는 공식 archive와 binding을 `make ci-setup`(`scripts/fetch-v8.mts`)으로만 가져오고, 이는 build 전에 SHA-256을 검증한다. cargo-nextest와 cargo-deny는 mise(`make install-tools`)에서만 오며 `mise.lock`이 고정한 digest를 쓴다. 긴 build는 전체 시간 제한 없이 명령과 진행을 보고하고, test case는 개별 시간 제한을 유지한다.
  Enforced by: `make verify-archive`, `make check-nextest`
- Own: 성능 주장은 기록된 한도와 함께 유지되는 benchmark(`make bench`)로 측정한다. 성능은 측정하되 test나 check를 실패시키지 않는다. 한도를 넘은 측정은 CI에서 `::warning::` annotation과 함께 warning을 출력한다. test는 동작(작업이 끝났다, 사건이 일어났다)을 assert하고 걸린 시간을 출력하며, `scripts/check-records.mts`는 측정된 시간에 대한 assertion을 거부한다.
  Enforced by: `make check-bench`, `make check-records`
- Own: symbolic link, 설정의 상대 경로, 반복 가능한 작업을 위한 임시 script를 쓰지 않는다. 반복 가능한 명령은 make target이거나 Git의 도구다. symbolic link와 상대 경로의 금지는 [PRINCIPLES.ko.md](PRINCIPLES.ko.md#repository-기준)의 S1이 제한하듯 이 저장소가 만드는 것에 적용된다.
  Enforced by: `make kit-check`
- Own: Linux는 GitHub CI의 Linux runner에서 검증한다. `.github/workflows/ci.yml`의 job `lint`와 `test`가 `ubuntu-24.04-arm`과 `ubuntu-24.04`에서 workspace를 lint, build, test하며, 각 CI target은 정확히 한 job에 있다. 어떤 check도 개발 machine의 container 도구를 실행하지 않는다.
  Enforced by: `make ci-lint`, `make ci-test`
- Own: application entry 파일을 생성하는 동시 test는 test마다 별도의 directory를 쓴다.
  Enforced by: `make check-nextest`

## Release

- Own: 모든 변경은 필수 check와 함께 merge queue를 거쳐 `main`에 도달하므로 `main`의 모든 commit은 전체 check를 통과했다. release는 `main`의 commit에 대한 tag이고, tag는 maintainer만 만들고 옮기고 push하며, pull request로 올리지 않는다.
  Enforced by: none, the live settings of GitHub are read with an administrator token that no CI job holds; `make github-ruleset-check` compares them on request.
- Own: version을 올리는 pull request `chore(release): Release X.Y.Z (#<checklist ID>)`는 `Cargo.toml`의 `[workspace.package]`와 workspace crate에 대한 모든 `=X.Y.Z` 요구에 version X.Y.Z를 쓰고, `CHANGELOG.md`와 `CHANGELOG.ko.md`의 `## Unreleased`를 `## X.Y.Z`로 바꾸며 그 위에 비어 있는 새 `## Unreleased`를 둔다. maintainer가 병합된 `main` commit에 `vX.Y.Z` tag를 달고 push한다. tag push는 kit의 release workflow `.github/workflows/release.yml`을 실행한다. 이는 tag된 commit이 `main`에 있고 check `push-gate`와 `ci-passed`가 통과했으며, tag의 version이 `config/release.json`의 모든 manifest에 있고 `CHANGELOG.md`에 section `## X.Y.Z`가 있는지 확인하고, GitHub Release를 초안으로 만들고, 각 crate가 tag에서 해석되는지 증명한 뒤 release를 공개한다. crate는 archive로 release하지 않고 git tag로 사용한다([workspace](docs/operations/workspace.ko.md)).
  Enforced by: `make release-versions`

## Idempotency

- Own: 어떤 결과도 시간이나 외부 세계에 의존하지 않는다. check는 live registry 질의(`npm outdated`, `npm audit`, `npm view`, 실행 시 해석되는 lock, 보안 advisory database)를 하지 않고, 실행 시 도구를 설치하지 않으며(`@latest`, rustup 자동 설치), 모든 도구를 `package.json`의 `packageManager`, `rust-toolchain.toml`, `mise.toml`과 `mise.lock`이 선언한 version으로 이 host와 CI에서 같은 방식으로 실행한다. advisory는 따로 검토한다(`make review-advisories`).
  Enforced by: `make kit-check`, `make toolchain-check`
- Own: test는 자기가 만들거나 자기 실행이 설치하고 검증한 것만 읽는다. 다른 실행이 남긴 build 출력, 설치된 package 복사본, target directory, 추적되지 않는 파일, 다른 checkout을 지목하는 compile 시점 고정 경로, checkout 사이에 공유되는 target directory를 읽지 않는다.
  Enforced by: `make check-nextest`
- Own: 다른 process가 읽는 출력은 한 단계로 게시한다. 임시 경로에 쓰고 rename하며, 지우고 제자리에서 다시 쓰지 않는다.
  Enforced by: none, which writes are read by other processes is judged by reading the code.
- Own: 실행은 모든 check를 보고한다. 단계는 상태를 누적하고, 실패한 단계는 자기 출력을 읽는 것만 건너뛰며, 어떤 recipe나 CI 단계도 첫 실패에서 멈추지 않는다(`|| exit`, `-k` 없는 `make`, `if: ${{ !cancelled() }}` 없는 CI 단계).
  Enforced by: `make check-tools`
- Own: 실패는 스스로 설명한다. 명령, 그 출력, 기대값과 실제값을 지목하며, 맨 timeout, "위의 오류", 도구의 오류 없는 종료 상태는 없다.
  Enforced by: none, whether a message explains itself is a judgement.
- Own: check는 실행되지 않고는 통과할 수 없다. 빈 선택, 빈 목록, 실행되지 않는 case는 실패한다.
  Enforced by: `make check-nextest`, `make test-markers-check`
- Own: test는 자기가 시작한 것을 수거한다. child는 자기 process group에서 실행되어 그 group을 kill하고 기다리며, 임시 directory는 assertion이 실패해도 guard가 제거한다.
  Enforced by: `make check-nextest`
- Own: version이나 context에 따라 형식이 달라지는 출력에서 결과를 읽지 않는다. 도구 결과는 고정된 형식의 기계가 읽는 report에서 오고, recipe는 `make -n`이 아니라 Makefile에서 읽는다.
  Enforced by: `make kit-check`
- Own: 추적되는 모든 파일은 `config/owner-checks.json`에 소유 check가 있고, 다른 crate가 의존하는 모든 crate는 `config/test-ownership.json`에 선언된 동작이 있다.
  Enforced by: `make owner-validate`, `make check-ownership`
- Own: 공유 상태는 한 번에 한 소유자를 갖는다. 동시 실행은 자기 directory, holder lock, content-addressed 불변 경로를 쓰며, lease 없는 공유 임시 directory, 출력 directory, database, port를 쓰지 않는다.
  Enforced by: none, which state is shared is a judgement.

## 기록

- Own: 기록은 commit log, 문서, 주석, i18n 텍스트, `CHANGELOG.md`(+ `.ko.md`)다. 기록은 현재 제품과 그 변경을 명시적인 주어, 동작, 목적어로, 은유, 의인화, 구어 없이 평이한 말로 적는다. 동작 이름을 직접 쓴다(create, publish, receive, register, remove, return, fail). 원인은 한 문장으로 적는다. envelope, publish, orphan, adopt, retire, dead, first-class, carries, speaks, answers 같은 단어는 제품 동작에 쓰지 않는다.
  Enforced by: `make check-records`, `make check-commit-records`
- Own: 기록은 결함을 ssr 자체에 관한 사실로 적는다. 입력, 동작, 기대 동작이다. 보고자, 출처, ssr 밖의 어떤 것도 지목하지 않고, code의 기원, 이식이나 migration 이력, 작성자나 도구 표기, 대화나 조사 이력을 담지 않으며, ssr에 관한 사실이 없는 문장은 삭제한다. 그런 맥락은 Git 밖의 로컬 memory에 둔다.
  Enforced by: none, whether a sentence names a source of a decision is a judgement that no tool makes.
- Own: 각 commit 전에, 변경을 소유한 Red와 Green check를 실행하며, `make owner-check`는 `config/owner-checks.json`의 변경된 파일의 소유자를 실행하고 `make owner-validate`는 소유자 없는 추적 파일을 실패시킨다.
  Enforced by: `make owner-validate`
