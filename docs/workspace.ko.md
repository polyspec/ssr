[English](workspace.md)

# 워크스페이스

Rust 1.98.1 워크스페이스는 AGENTS.md에 적힌 여덟 크레이트로 구성한다. 모든 크레이트는 edition 2024와 버전
0.0.1을 쓴다. 워크스페이스 잠금 파일은 의존성 해석 결과를 기록한다. 직접 의존성은 공식 배포 정보를 확인한 뒤
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
Rust 테스트는 assertion이 실패해도 guard로 임시 디렉터리를 지운다. `ssr-server`의 어떤 테스트보다 먼저
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
(`crates/ssr-build/tests/fixture/mod.rs`) 생성 entry를 거기에 쓰며, 설치본을 `BuildConfig::dependencies`로 build하고
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
