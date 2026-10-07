[English](workspace.md)

# 워크스페이스

Rust 1.98.1 워크스페이스는 AGENTS.md에 적힌 여덟 크레이트로 구성한다. 모든 크레이트는 edition 2024와 버전
0.0.2를 쓴다. 워크스페이스 잠금 파일은 의존성 해석 결과를 기록한다. 직접 의존성은 공식 배포 정보를 확인한 뒤
정확한 버전으로 지정한다.

`make check`는 문서 쌍과 링크, 기록, 용어, Python 도구 테스트, Rust 형식, Clippy 경고, 의존성 금지 목록과
라이선스·출처, Rust 단위 테스트를 검사한다. 보안 공지는 시간에 따라 바뀌는 database에서 오므로 `make check`는 이를
읽지 않고, `make review-advisories`가 workspace, engine 검증, build 검증의 보안 공지를 검토한다.
`tools/tool-versions.json`은 검사, build, test가 실행하는 모든 도구를 version 보고의 정확한 첫 줄로, 그것이 없는 도구는
실행 파일의 SHA-256으로 적는다. Python은
minor version 3.9로 고정하고, 검사는 실행 중인 patch release를 출력한다. `tools/tool_versions.py`는 설치된 도구를 `make check`와 `make rerun-failed`의 어떤 단계보다
먼저, `tools/check.py`에서, 다른 make target의 첫 command로, 그리고 Python은 pre-push hook에서 비교하고, 기대 보고와 실제
보고를 적고 실패한다. 모든 make 호출은 `RUSTUP_AUTO_INSTALL=0`을 설정하므로 없는 toolchain은 설치되지 않고 실패한다. Rust 테스트는 cargo-nextest 0.9.146으로 실행한다. nextest 설정은
각 테스트를 30초 뒤 종료하고 시작 및 결과와 경과 시간을 출력한다. 테스트 도구가 없으면 명령은 실패한다.
Python 도구 테스트는 `tools/run_tests.py`로 각자 자기 process group에서 30초 제한으로 실행한다. 제한을 넘은
사례는 출력과 함께 보고하고 그 process group 전체를 죽이므로 사례가 시작한 process가 계속 실행되지 않는다.
Rust 테스트는 assertion이 실패해도 guard로 임시 디렉터리를 지운다. `polyspec-ssr-server`의 어떤 테스트보다 먼저
nextest setup script `build-programs`(`tools/build_programs.py`, 고정된 nextest의 실험 기능)가 development와
socket 테스트가 실행하는 `development_process`와 `socket_process` 예제를 빌드하고 `SSR_DEVELOPMENT_PROCESS`와
`SSR_SOCKET_PROCESS`로 알린다. 테스트는 이 변수에서만 프로그램을 실행하므로 test target 하나의 실행이 이전
소스로 빌드된 프로그램을 실행하지 않는다. 빌드 검증 workspace `tools/build-probe`는 자기 nextest 설정을 가진다.
`make check`는 `tools/full_run.py`로 전체 묶음을 실행한다: setup 단계 `CHECK_SETUP`(archive 검사와
fixture 설치) 다음에 `CHECK_TARGETS`의 각 target을 자기 make target으로 실행한다. 어떤 단계보다 먼저, `docs/checklist.md`의
항목(하위 항목 포함)이 `[~]`인 동안(각 항목을 ID와 제목으로 적는다), 추적하는 file에 commit하지 않은 변경이 있는
동안, 추적되지도 무시되지도 않는 file이 있는 동안(각 file을 적는다. 단계는 tree에 없는 그런 file을 읽을 수 있다), pre-push hook이 설치되지 않은 동안(아래 참고), 그리고 `var/full-run.json`이 같은 tree(`git rev-parse HEAD^{tree}`)의 전체 실행을 기록하고 있을 때(그 실행을
적는다) 이유와 0이 아닌 종료 상태로 실행을 거부한다. 기록은 tree, commit, 결과, 통과하지 못한 target과 각 단계의
시각을 담는다. 첫 단계 전과 각 단계의 시작과 끝마다 쓰므로 강제 종료된 실행은 `incomplete`로 남는다. 실패한
target이 있어도 실행은 멈추지 않는다. 모든 setup 단계는 앞선 단계가 실패해도 실행된다. `CHECK_NEEDS`는
각 target이 읽는 setup 단계를 적고, setup 단계가 실패한 target은 그 단계와 함께 `skipped`로 기록되며 다른 모든
target은 실행된다. `make verify-build`는 각 검사를 실행하고 실패한 검사마다 이름을 적는다. `make rerun-failed`는 setup 단계와 그 기록에서 통과하지 못한 target만 다시
실행하며, 현재 tree의 기록이 없거나 모든 target이 통과했으면 거부된다. 새 checkout에는 기록이
없다. 두 진입점 모두 테스트가 읽는 build-probe fixture를 설치하므로, `python3 -m tools.holder_lock run check`가
모든 단계 동안 checkout lock `var/locks/check.lock`을 잡는다. 같은 checkout의 두 번째 실행은 holder의 checkout,
pid, process 시작 시각과 함께 거부된다. target 없는 make는 `make check`를 실행한다. 테스트는 checkout의 build-probe fixture에 쓰지 않는다. 설치본을 읽는 테스트보다 먼저 nextest setup script
`install-packages`(`tools/install_packages.py`)가 `tools/build-probe/tests/fixtures`의 package를 잠금 파일마다 한 번
불변의 `var/packages/<package.json과 package-lock.json의 SHA-256>`에 설치해 이름 바꾸기 한 번으로 게시하고, 그것을
`SSR_PACKAGES`로, 소스를 `SSR_FIXTURES`로 알린다. 각 테스트는 소스를 자기만의 새 임시 루트로 복사하고
(`crates/polyspec-ssr-build/tests/fixture/mod.rs`) 생성 entry를 거기에 쓰며, 설치본을 `BuildConfig::dependencies`로 build하고
끝날 때 루트를 지운다. browser script는 `playwright-core`를 `SSR_PACKAGES`에서 가져오고, browser 사례는 host의 browser가 아니라 이
`playwright-core`가 고정한 chromium build를 같은 directory에 설치해 `SSR_BROWSER`로 알린 것으로 실행한다. script의 filter는 설치본을
읽는 test binary를 정확히 적고, `tools/check.py`가 이를 검사한다.

push는 `docs/checklist.md`의 항목(하위 항목 포함)이 `[~]`가 아닐 때만 한다. 추적되는 pre-push hook
`.githooks/pre-push`는 `python3 -m tools.push_gate hook`을 실행한다. 이것은 push되는 각 ref의 tip commit과 작업 트리의
체크리스트를 `tools/full_run.py`의 parser로 읽고, 진행 중인 각 항목을 원격 ref, commit, ID, 제목과 함께 적고
종료 상태 1로 push를 거부한다. 항목은 한 commit에서 진행 중으로 기록되고 다음 commit에서 완료되므로
push의 앞선 commit은 읽지 않는다. push되는 tip commit에 `docs/checklist.md`가 없거나 Git 오류가 나도 push를 거부한다.
원격 branch를 지우는 push는 commit을 보내지 않으므로 작업 트리만 읽는다. 모든 make 실행은 `core.hooksPath`가
`.githooks`와 다르면 그것으로 설정하므로, make를 실행한 모든 checkout에서 hook이 실행된다. `make hooks`는 그것을
설정하고 `make hooks-check`를 실행하며, 이것은 `core.hooksPath`가 `.githooks`이고 hook이 실행 가능하지 않으면
실패한다. `tools/check.py`와 `make check`의 guard도 같은 검사를 실행한다. workflow
`.github/workflows/push-gate.yml`은 모든 push의 push된 tip commit과 모든 pull request의 head commit(merge commit이 아님)에서 job
`push-gate`로 `python3 -m tools.push_gate commit HEAD`를 실행한다. 진행 중인 각 항목을 error annotation과 job
summary에 적고 실패하며, commit이 hook을 mode 100755로 추적하지 않을 때도 실패한다.
job은 `actions/setup-python`의 Python 3.9로 `ubuntu-24.04-arm`에서 실행되고, action은 commit에 고정되며, push 검사 전에
선언된 Python을 검사한다. 이것은 `make push-records`(`python3 tools/check.py records`)도 실행한다. 이는 추적되는 파일만
읽는 `tools/check.py`의 검사로, 문서 쌍과 link, checklist 표시와 문장과 ID, 기록 단어, React 문서, home directory로의
경로, 시간 assertion이다. ruleset이 `main`에 이 job을 요구하므로 기록 검사가 실패하는 commit은 `main`에 오르지 않는다.
CI는 `make ci-records`(`python3 tools/check.py ci`)를 실행한다. 이는 runner가 선언된 version으로 설치하는 도구인
Python과 rustc의 version으로 `tools/check.py`의 모든 검사를 실행한다.

`.github/workflows/ci.yml`은 `ubuntu-24.04-arm`과 `ubuntu-24.04` 각각에서 job 두 개를 실행한다. job `lint`는
`make ci-setup`과 `make ci-lint`, 즉 target `CI_LINT_TARGETS`(`ci-records`, `check-fmt`, `check-clippy`, `check-deny`)를
실행하고, job `test`는 `make ci-setup`과 `make ci-test`, 즉 target `CI_TEST_TARGETS`(`check-examples`, `ci-nextest`)를
실행한다. 따라서 모든 CI target은 각 architecture에서 정확히 한 job에서 실행되고 두 job은 동시에 실행된다. 각 job은
log와 요약을 `ci-<job>-<runner>`로 올린다.
`make ci-setup`은 고정된 toolchain, host target의 공식 V8 입력(`tools/fetch_v8.py`), host target용 cargo-nextest 0.9.146과
cargo-deny 0.20.2의 release binary를 설치한다. `tools/tool-versions.json`은 `releases`에 각 release archive의 URL, 그
SHA-256, 그 안의 실행 파일 경로를 선언하고, `tools/fetch_tools.py`는 각 archive를 내려받아 SHA-256이 다르면 아무것도
설치하지 않고 실패하며, 실행 파일을 한 번의 rename으로 `$CARGO_HOME/bin`에 설치한다. `make ci-lint`와 `make ci-test`는 먼저 Python, rustc,
cargo-nextest, cargo-deny의 version 보고를 `tools/tool-versions.json`과 비교한다. Cargo는
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
RESULTS='${{ toJSON(needs) }}'`를 실행한다. `python3 -m tools.ci_run --passed`는 need로 가진 모든 job의 결과를 출력하고,
하나라도 `success`가 아니면 실패한다. `ci.yml`에 추가한 job은 `needs`에 넣으므로 필수 check가 그 job을 포함한다.
`tools/test_ci.py`는 `ci-passed`가 마지막 job이 아니거나, `if: ${{ always() }}`가 없거나, 다른 모든 job을 need로 가지지
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
선언 값으로 적고 실패한다. 둘 다 저장소 administration 권한이 있는 인증된 `gh`가 필요하다. `tools/test_github_ruleset.py`는
가짜 `gh`로 도구를 실행한다.

릴리스는 `main`의 commit에 붙인 tag `vX.Y.Z`이고([릴리스 절차](../AGENTS.ko.md)), `crates/`의 crate를 버전 X.Y.Z로
릴리스한다. crate는 archive로 릴리스하지 않고 git tag로 사용한다. `cargo package`는 `polyspec-ordered-json` 같은 git
의존성을 해석되지 않는 crates.io 요구로 바꾸기 때문이다. tag의 push는 `.github/workflows/release.yml`(`on: push: tags:
['v*', '**/v*']`, 권한 `contents: write`, 환경 변수 `TAG`의 tag. tag filter에서 `*`는 `/`와 맞지 않으므로 `**/v*`가
어느 깊이의 tag `<directory>/vX.Y.Z`든 포함한다)을 실행한다. 그 step은 Python을 `tools/tool-versions.json`과 비교하는
`make release-setup`을
실행한 뒤, 다음 순서로 `tools/release.py`를 실행하고 첫 실패에서 멈춘다. `make release-verify`는 tag된 commit이
`origin/main`의 조상이고 그 commit의 최신 check run `push-gate`와 `ci-passed`(`gh api
repos/<repository>/commits/<sha>/check-runs`)가 결론 `success`로 완료되었는지 확인하며 테스트를 다시 실행하지 않는다.
`make release-versions`는 `Cargo.toml`의 `[workspace.package]`, 자기 버전을 선언한 모든 crate manifest, workspace
crate에 대한 모든 요구에 X.Y.Z가 있고 `docs/changelog.md`에 section `## X.Y.Z`가 있는지 확인하며, 파일마다 두 값을
적는다. `make release-assets`는 릴리스 asset인 npm tarball과 Composer zip만 비어 있는 `var/release/assets`에 만들며,
저장소에는 그런 패키지가 없다. `make release-publish`는 archive 없이 `gh release create <tag> --verify-tag --title <tag>
--notes-file <notes>`를 실행한다. notes는 section X.Y.Z가 GitHub release body의 한도인 125000자 이하이면 그 section이고,
아니면 한 줄 `The changes of X.Y.Z are listed in
[CHANGELOG.md](https://github.com/polyspec/ssr/blob/<tag>/docs/changelog.md#<점을 뺀 version>).`이다. 저장소에는 Go 모듈이 없으므로 tag `<directory>/vX.Y.Z`는 실패한다.
`tools/test_release.py`는 `gh`의 fake로
각 step을 실행하고, `tools/test_ci.py`는 trigger, 권한, step의 순서를 요구한다.

`make bench`는 Cargo 벤치마크를 실행한다. 유지할 측정과 한도는 S-11에 명시한다.

Rust 1.98.1은 고정한 [Rust 배포](https://github.com/rust-lang/rust/releases/tag/1.98.1)이다.
cargo-nextest 0.9.146은 고정한 [테스트 실행기 배포](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146)다.
의존성 검사는 MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib,
MPL-2.0을 허용한다. 다른 라이선스는 검사에서 실패한다.
[xxhash-rust 0.8.18](https://crates.io/crates/xxhash-rust/0.8.18)의 메타데이터에는 BSL-1.0이
명시되어 있다. [dragonbox_ecma 0.1.12](https://crates.io/crates/dragonbox_ecma/0.1.12)의
메타데이터에는 LLVM 예외가 적용된 Apache-2.0 또는 BSL-1.0이 명시되어 있다. 의존성 검사는
xxhash-rust 0.8.18에만 BSL-1.0을, dragonbox_ecma 0.1.12에만 LLVM 예외가 적용된
Apache-2.0을 허용한다.
