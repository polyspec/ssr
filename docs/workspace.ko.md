<!-- doc-id: workspace -->
<!-- source-sha256: 834a37f3dbdf9948682b0f9639768a99e977a3ad84ab74e9ab131ae85a8c267b -->
[English](workspace.md)

# 워크스페이스

Rust 1.99.0 워크스페이스는 AGENTS.md에 적힌 아홉 패키지로 구성하며, 각 패키지는 `packages/<name>`에 있고
(`Cargo.toml`의 `members = ["packages/*"]`) 이름은 `polyspec-ssr-<name>`이다. 모든 패키지는 edition 2024와 버전
0.0.2를 쓴다. 워크스페이스 잠금 파일은 의존성 해석 결과를 기록한다. 직접 의존성은 공식 배포 정보를 확인한 뒤
정확한 버전으로 지정한다. `Cargo.toml`의 `[workspace.lints]`는 `unsafe_code`와 `clippy::allow_attributes_without_reason`을
deny하고, 모든 패키지는 `[lints] workspace = true`로 이 lint를 따른다.

`make check`는 문서 쌍과 링크, 기록, 용어, `scripts/`와 `tests/`의 TypeScript의 type과 lint(`tsconfig.json`으로 실행하는
`make typecheck`, kit의 `make lint-ts`), 도구 테스트, 테스트 소유권, Rust 형식, Clippy 경고, 의존성 금지 목록과
라이선스·출처, Rust 테스트를 검사한다. 보안 공지는 시간에 따라 바뀌는 database에서 오므로 `make check`는 이를
읽지 않고, `make review-advisories`(`cargo deny check advisories`)가 workspace의 보안 공지를 검토한다.
`mise.toml`은 cargo-nextest 0.9.146과 cargo-deny 0.20.2를 선언하고, `mise.lock`은 모든 platform의 release binary를
고정하며, `make install-tools`가 mise로 이를 설치하고 `make toolchain-check`가 version 보고를 선언과 비교한다. Node.js, npm, Rust는
`mise.toml`, `package.json`의 `packageManager`, `rust-toolchain.toml`에 kit의 pin으로 선언되고, `make toolchain-check`가
이를 비교한다. 모든 make 호출은 `RUSTUP_AUTO_INSTALL=0`을 설정하므로 없는 toolchain은 설치되지 않고 실패한다. Rust 테스트는 cargo-nextest 0.9.146으로 실행한다. nextest 설정
`.config/nextest.toml`은 각 테스트를 30초 뒤 종료하고 시작 및 결과와 경과 시간을 출력한다. 테스트 도구가 없으면 명령은
실패한다. 도구 테스트 `tests/tools/<name>.test.mts`는 `scripts/`의 TypeScript 도구와 저장소 설정(workflow, Makefile,
`Cargo.toml`, `deny.toml`)을 테스트한다. `make check-tools`는 kit의 test runner(`node $(KIT)/run-tests.$(KIT_EXT) node`)로
모든 file이나 `TESTS`의 file을 실행하고, `make tool-test-<name>`은 file 하나를 실행한다. runner는 각 테스트의 시작과
끝을 경과 시간과 함께 출력하고 모든 테스트에 자기 timeout을 준다. Rust 테스트는 assertion이 실패해도 guard로 임시 디렉터리를 지운다. `polyspec-ssr-server`의 어떤 테스트보다 먼저
nextest setup script `build-programs`(`scripts/build-programs.mts`, 고정된 nextest의 실험 기능)가 development와
socket 테스트가 실행하는 `development_process`와 `socket_process` 예제를 빌드하고 `SSR_DEVELOPMENT_PROCESS`와
`SSR_SOCKET_PROCESS`로 알린다. 테스트는 이 변수에서만 프로그램을 실행하므로 test target 하나의 실행이 이전
소스로 빌드된 프로그램을 실행하지 않는다.
`make check`는 kit의 guard(package `@polyspec/kit`의 `full-run`)로 전체 묶음을 실행한다. `CHECK_TARGETS`의 각
target(kit 검사 `kit-check`, `toolchain-check`, `layout-check`, `documents-check`, `owner-validate`, `hooks-check`,
그다음 이 저장소의 target)은
`make -k <target>`로 끝까지 실행된다. 어떤 target보다 먼저, `docs/plans/execution-checklist.md`의 항목(하위 항목
포함)이 `[~]`인 동안, 추적하는 file에 commit하지 않은 변경이 있는 동안, 추적되지도 무시되지도 않는 file이 있는 동안,
pre-push hook이 설치되지 않은 동안(아래 참고), checkout의 다른 실행이 lock `var/full-run.lock`을 잡고 있는 동안, 그리고
`var/full-run.json`이 같은 tree(`git rev-parse HEAD^{tree}`)의 전체 실행을 기록하고 있을 때 이유와 0이 아닌 종료
상태로 실행을 거부한다. 기록은 tree, 결과, 각 target의 상태와 시각을 담고, 각 target 전후에 쓰므로 멈춘 실행은
`incomplete`로 남는다. 실패한 target이 있어도 실행은 멈추지 않는다. V8 입력이나 fixture package를 읽는 target은
그 setup 단계 `verify-archive`(`scripts/verify-archive.mts`)나 `check-fixtures`(`scripts/install-packages.mts`)를 선행
조건으로 적으므로, 실패한 setup 단계는 그것을 읽는 target만 실패시킨다. `make rerun-failed`는 그 기록에서
통과하지 못한 target만 다시 실행하며, 현재 tree의 기록이 없거나 모든 target이 통과했으면 거부된다. 새 checkout에는
기록이 없다. target 없는 make는 `make check`를 실행한다. 테스트는 checkout의 fixture directory에 쓰지 않는다. 설치본을 읽는 테스트보다 먼저 nextest setup script
`install-packages`(`scripts/install-packages.mts`)가 `packages/build/tests/fixtures`의 package를 잠금 파일마다 한 번
불변의 `var/packages/<package.json과 package-lock.json의 SHA-256>`에 설치해 이름 바꾸기 한 번으로 게시하고, 그것을
`SSR_PACKAGES`로, 소스를 `SSR_FIXTURES`로 알린다. 각 테스트는 소스를 자기만의 새 임시 루트로 복사하고
(`packages/build/tests/fixture/mod.rs`) 생성 entry를 거기에 쓰며, 설치본을 `BuildConfig::dependencies`로 build하고
끝날 때 루트를 지운다. browser script는 `playwright-core`를 `SSR_PACKAGES`에서 가져오고, browser 사례는 host의 browser가 아니라 이
`playwright-core`가 고정한 chromium build를 같은 directory에 설치해 `SSR_BROWSER`로 알린 것으로 실행한다. script의 filter는 설치본을
읽는 test binary를 정확히 적고, `make check-fixture-filter`(`scripts/install-packages.mts check-filter`)가 이를
검사한다. 이 검사는 filter를 smol-toml로 `.config/nextest.toml`에서 읽고 workspace의 test binary를 `cargo metadata`에서
읽는다.

`make check-ownership`은 workspace의 member가 아닌 package `tests/test-ownership`으로 `config/test-ownership.json`의
테스트 소유권([테스트 소유권](test-ownership.ko.md))을 검사한다(`cargo run --locked --manifest-path
tests/test-ownership/Cargo.toml -- <root>`). `make tool-test-test-ownership`은 그 테스트, 형식, Clippy를 실행한다.
`config/layout.yml`은 kit의 `make layout-check`를 위해 저장소의 folder를 선언한다: cargo-nextest와 cargo-deny가 자기
관례로 읽는 root의 `.config/`와 `deny.toml`, `packages/`의 package, `tests/tools`, 그리고 Rust 소스를 syn으로 읽으므로
Rust로 작성한 `tests/test-ownership`이다.

push는 `docs/plans/execution-checklist.md`의 항목(하위 항목 포함)이 `[~]`가 아닐 때만 한다. kit의 `make hooks`가 쓰는
추적되는 pre-push hook `.githooks/pre-push`는 `make push-gate-hook`을 실행한다. kit의 push 검사는 push되는 각 ref의 tip
commit과 작업 트리의 체크리스트를 `config/checklist.json`으로 읽고 진행 중인 각 항목을 적고 push를 거부하며, push되는
commit의 message를 `config/commits.json`과 비교한다. 모든 make 실행은 `core.hooksPath`가 `.githooks`와 다르면 그것으로
설정하므로, make를 실행한 모든 checkout에서 hook이 실행된다. `make hooks-check`는 `core.hooksPath`가 `.githooks`이고
hook이 kit의 것이며 실행 가능하지 않으면 실패하고, `make check`의 guard도 같은 검사를 실행한다. workflow
`.github/workflows/push-gate.yml`은 모든 push의 push된 tip commit과 모든 pull request의 head commit(merge commit이 아님)에서
job `push-gate`로 `make toolchain-check`가 읽는 `mise.lock`의 도구와 `rust-toolchain.toml`의 Rust toolchain을
설치하고(`make install-tools`, kit K151) `make ci-push`를 실행한다. 이는 kit의 `make ci-targets`로 target `CI_PUSH_TARGETS`를 실행한다:
`toolchain-check`, `push-gate-commit`(`HEAD`에 대한 kit의 push 검사로, 진행 중인 각 항목을
error annotation과 job summary에 적고 실패하며 commit이 hook을 mode 100755로 추적하지 않을 때도 실패한다),
`check-records`(`node scripts/check-records.mts`, 추적되는 파일의 검사: 기록 단어, React 문서, home directory로의 경로,
시간 assertion), `check-commit-records`(`node scripts/check-records.mts --range $(RANGE)`, `RANGE`의 commit message의 기록
단어. job은 `RANGE`를 event가 가져오는 commit으로 정한다: push는 그것이 대체한 tip 뒤, 새 branch면 default branch 뒤의
commit, pull request와 merge group은 base 뒤의 commit이다. checkout은 range가 해석되도록 전체 history를 가져오고, push된
commit은 다시 읽지 않으며, `RANGE`가 없으면 usage와 함께 실패하고 비었거나 해석되지 않는 range는 실패한다. push되는 range가
없는 `make check`의 target이 아니며, kit의 파일인 pre-push hook은 아직 이를 실행하지 않는다), `typecheck`, `lint-ts`. job은 `mise.toml`의 Node.js로 `ubuntu-24.04-arm`에서 실행되고, action은
commit에 고정되어 주석에 version을 적으며, Python이나 Rust가 필요 없다. ruleset이 `main`에 이 job을 요구하므로 기록 검사가 실패하는
commit은 `main`에 오르지 않는다. kit의 `make documents-check`가 문서 쌍, revision, link와 체크리스트를 검사한다.

`.github/workflows/ci.yml`은 `ubuntu-24.04-arm`과 `ubuntu-24.04` 각각에서 job 두 개를 실행한다. 각 job은 먼저
`packageManager`의 npm과 `@polyspec/kit`을 포함한 `package-lock.json`의 package를 설치한다(`make install-tools TOOLS=npm`,
`make install-packages`), 이어서 `mise.lock`의 도구를 설치한다(`make install-tools`). job `lint`는
`make ci-setup`과 `make ci-lint`, 즉 target `CI_LINT_TARGETS`(`check-fixture-filter`, `check-fmt`, `check-clippy`, `check-deny`)를
실행하고, job `test`는 `make ci-setup`과 `make ci-test`, 즉 target `CI_TEST_TARGETS`(`check-examples`, `ci-nextest`)를
실행한다. 따라서 모든 CI target은 각 architecture에서 정확히 한 job에서 실행되고 두 job은 동시에 실행된다. 두 job은
kit의 `make ci-targets`로 실패와 관계없이 target을 실행하고, 각 job은 target별 log와 요약인 `var/report/ci-targets`를
`ci-<job>-<runner>`로 올린다.
`make install-tools`는 `rust-toolchain.toml`의 Rust toolchain을(kit K151), `make ci-setup`은 host target의 공식 V8
입력(`scripts/fetch-v8.mts`)을 설치한다.
`make ci-lint`와 `make ci-test`는 먼저 `make toolchain-check`를 실행하며, 이 검사는 cargo-nextest와 cargo-deny도 확인한다. Cargo는
check target을 `BUILD_JOBS`개의 job으로 빌드한다. 기본값은 1이므로 로컬 빌드는 host의 다른 processor를 다른 작업에
남기고, `make ci-lint`와 `make ci-test`는 `BUILD_JOBS`를 runner의 processor 수(`nproc`)로 설정한다.
이 job들이 AArch64와 x86_64에서 저장소의 Linux 검증이다. `check-clippy`는 workspace의 모든 target을 lint하고,
`check-examples`는 example을 build하며, `ci-nextest`는 workspace의 모든 test를 실행한다. 어떤 make target도 개발
machine의 container 도구를 실행하지 않는다.

모든 변경은 pull request와 merge queue를 거쳐 `main`에 들어간다. 이 저장소의 어떤 명령도 `main`을 push하지 않는다.
branch는 GitHub의 표준 명령이나 GitHub UI로 게시한다.

```sh
git push origin HEAD:refs/heads/<branch>
gh pr create --base main --head <branch> --fill
gh pr merge <branch> --auto --rebase
```

`.github/ruleset.json`의 GitHub ruleset `main`은 enforcement `active`로 `refs/heads/main`에 적용되고 bypass actor가
없으므로 관리자에게도 적용된다. 그 rule은 다음과 같다. `pull_request`: 변경은 승인이 필요 없는 pull request로 들어오고,
`gh pr merge --auto`가 스스로 고른 method로 auto-merge를 요청하므로 모든 merge method를 허용한다. `merge_queue`: queue는
method `REBASE`와 grouping strategy `ALLGREEN`으로 merge하고, 한 번에 최대 5개 항목을 build하고 merge하며 더 기다리지
않고, `check_response_timeout_minutes`는 GitHub의 최댓값인 360이다. `required_linear_history`, `non_fast_forward`,
`deletion`: `main`의 merge commit, force-push, 삭제가 없다. `required_status_checks`: 정확히 GitHub Actions
app(integration 15368)의 check `push-gate`와 `ci-passed`, 즉 `.github/workflows/push-gate.yml`의 job과
`.github/workflows/ci.yml`의 마지막 job이다. job `ci-passed`는 `ci.yml`의 다른 모든 job을 need로 가지고, 그중 하나가
실패하거나 skip되거나 취소되어도 그 모든 job 뒤에 실행되며(`if: ${{ always() }}`), `make ci-passed
RESULTS='${{ toJSON(needs) }}'`를 실행한다. kit의 이 target은 need로 가진 모든 job의 결과를 출력하고, 하나라도
`success`가 아니면 실패한다. `ci.yml`에 추가한 job은 `needs`에 넣으므로 필수 check가 그 job을 포함한다.
`tests/tools/workflows.test.mts`는 `ci-passed`가 마지막 job이 아니거나, `if: ${{ always() }}`가 없거나, 다른 모든 job을 need로 가지지
않거나, `push-gate`와 다른 runner에서 실행되거나, 다른 step을 실행하면 실패한다. `git push origin <commit>:main`의 직접 push는 `GH013: Repository
rule violations found`로 거부된다. `gh pr merge --auto`는 pull request의 필수 check가 통과하면 pull request를 merge
queue에 넣는다. queue는 이를 branch `gh-readonly-queue/main/pr-<번호>-<sha>`의 merge group으로 `main` 위에 rebase하고,
두 workflow가 그 commit에서 실행되며(`merge_group`), check가 통과하면 queue가 `main`을 정확히 그 commit으로 옮긴다.
check가 실패하면 pull request는 queue에서 빠지고 `main`은 움직이지 않는다. `ci.yml`은 pull request, merge group, 수동 실행(`workflow_dispatch`)에서
실행되고 pull request에 새 push가 있을 때만 실행을 취소한다. `push-gate.yml`은 queue의 branch를 뺀 모든 push에서도
실행된다. merge된 pull request의 branch는 지워진다(`delete_branch_on_merge`). rebase는 merge된 commit에 새 hash를 주므로
`git pull --rebase`가 queue가 merge한 로컬 commit을 버린다. `make github-ruleset`은 선언된 저장소 설정(`allow_rebase_merge`,
`allow_auto_merge`, `delete_branch_on_merge`)을 바꾸고 선언된 이름의 ruleset을 다르면 만들거나 갱신하며 다시 비교한다.
`make github-ruleset-check`는 아무것도 바꾸지 않고, 설정이 다르거나 live ruleset이 없거나 다르면 각 field를 live 값과
선언 값으로 적고 실패한다. 둘 다 저장소 administration 권한이 있는 인증된 `gh`가 필요하다. `tests/tools/github-ruleset.test.mts`는
가짜 `gh`로 `scripts/github-ruleset.mts`를 실행한다.

릴리스는 `main`의 commit에 붙인 tag `vX.Y.Z`이고([릴리스 절차](../AGENTS.ko.md)), `packages/`의 package를 버전 X.Y.Z로
릴리스한다. crate는 archive로 릴리스하지 않고 git tag로 사용한다. `cargo package`는 `polyspec-ordered-json` 같은 git
의존성을 해석되지 않는 crates.io 요구로 바꾸기 때문이다. tag의 push는 kit의 release workflow인 `.github/workflows/release.yml`을 실행한다. `make kit-sync`가 이를 복사하고 `make
kit-check`가 비교한다. 그 step은 kit의 package를 설치한 뒤 다음 순서로 실행하고 첫 실패에서 멈춘다: `make
release-tools`, `make release-verify`(tag된 commit이 `origin/main`의 조상이고 그 commit의 최신 check run `push-gate`와
`ci-passed`가 `success`로 끝났다), `make release-versions`(`config/release.json`의 모든 manifest에 X.Y.Z가 있고
`docs/changelog.md`와 `docs/changelog.ko.md`에 section `## X.Y.Z`가 있다), `make release-assets`(`config/release.json`이
package를 선언하지 않으므로 archive가 없다), `make release-publish`(section을 notes로 한 GitHub Release 초안), `make
release-proof`(각 crate를 tag에서 새 crate의 git 의존성으로 해석하고 `cargo check`가 통과한다, `config/release.json`의
`proof.gitTag`), `make release-undraft`. `config/release.json`은 workspace manifest를 version으로, 각 crate manifest를
git tag로 릴리스되는 것으로, 다른 모든 추적 manifest를 이유와 함께 릴리스되지 않는 것으로 선언한다. `make
release-coverage`는 분류되지 않은 manifest에서 실패한다.

`make bench`는 렌더 벤치마크를 실행한다. `cargo run --locked -p polyspec-ssr-runtime --example bench --features bench`가
example `bench`를 빌드해 실행하고, `make check-bench`가 그 테스트를 실행한다. 유지할 측정과 한도는 S-11에 명시한다.

Rust 1.99.0은 고정한 [Rust 배포](https://github.com/rust-lang/rust/releases/tag/1.99.0)이고, Node.js 26.12.0은 `mise.toml`이 고정하고 `make install-tools TOOLS=npm`이 var/tools에 설치하는 Node.js 배포다. 모든 Cargo 패키지는 `rust-version = "1.99.0"`을 선언한다.
cargo-nextest 0.9.146은 고정한 [테스트 실행기 배포](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146)다.
의존성 검사는 MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib,
MPL-2.0을 허용한다. 다른 라이선스는 검사에서 실패한다.
[xxhash-rust 0.8.18](https://crates.io/crates/xxhash-rust/0.8.18)의 메타데이터에는 BSL-1.0이
명시되어 있다. [dragonbox_ecma 0.1.12](https://crates.io/crates/dragonbox_ecma/0.1.12)의
메타데이터에는 LLVM 예외가 적용된 Apache-2.0 또는 BSL-1.0이 명시되어 있다. 의존성 검사는
xxhash-rust 0.8.18에만 BSL-1.0을, dragonbox_ecma 0.1.12에만 LLVM 예외가 적용된
Apache-2.0을 허용한다.
