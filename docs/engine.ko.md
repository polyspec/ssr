[English](engine.md)

# 엔진 검증

## 버전과 아카이브 획득

`verification/engine`의 검증 프로그램은 `deno_core =0.412.0`,
`deno_webidl =0.259.0`, `deno_web =0.290.0`을 고정한다. 커밋한 잠금 파일은
`deno_v8 0.4.0`과 `v8 150.4.0`을 선택한다. 이 프로그램은 `deno_webidl` 및
`deno_web` 확장을 사용하여 `JsRuntime`을 생성하므로 최종 링크에 필요한 세
크레이트가 모두 포함된다.

`deno_v8 0.4.0`의 기본 `v8` 기능은 `v8 150.4.0`을 선택한다.
[`v8` 빌드 스크립트](https://docs.rs/crate/v8/150.4.0/source/build.rs)는
`https://github.com/denoland/rusty_v8/releases/download/v150.4.0/`에서
사전 빌드 아카이브를 받는다. 선택된 기능에 `simdutf`가 포함되므로 아카이브
이름은 `librusty_v8_simdutf_release_<target>.a.gz`이다. 환경 변수
`RUSTY_V8_ARCHIVE`로 절대 경로의 로컬 아카이브를 선택한다. 빌드 스크립트는
이 경로의 SHA-256을 검사하지 않으므로, `tools/verify_engine.py`가 압축된
아카이브의 SHA-256을 확인한 뒤 Cargo를 실행한다.

| 대상 | 압축 아카이브 SHA-256 |
| --- | --- |
| `aarch64-apple-darwin` | `5aeffd8d5a0c1b79ac1d70af83d5b19099655fd9c645a794dc43f101f779838c` |
| `x86_64-apple-darwin` | `a750271fec6b211457ed0a5cf7d2eab1924b265621a82da86ab959d6ff0823e4` |
| `aarch64-unknown-linux-gnu` | `539e283815a396a5796f32858b42e517b858ebaaeaaad05d03290ee8c864a527` |
| `x86_64-unknown-linux-gnu` | `f48762ca10d1f1fc605a441c5ae430ec8ce1e9e80f14d78fbc42cb878c30b476` |

`python3 tools/verify_engine.py <target> --fetch`를 실행하면 없는 아카이브를
다운로드하고 SHA-256을 검사한 뒤 검증 프로그램을 `cargo build --locked
--target`으로 완전히 빌드한다. `--fetch`를 생략하면 `var/v8`에 아카이브가
있어야 한다. 아카이브 누락, 해시 변경, 다운로드 실패, 빌드 실패 및 시간
초과는 오류로 처리한다.

## 빌드 증거

빌드 호스트는 macOS arm64이며 Rust 1.98.1과 Zig 0.16.0을 사용했다. 다음
결과는 대상별 전체 빌드·링크와 대상 운영체제에서의 실행을 구분한다.

| 대상 | 전체 빌드·링크 | 실행 | 증거 |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | 통과 | 호스트에서 통과 | Mach-O arm64 실행 파일 |
| `x86_64-apple-darwin` | 통과 | Rosetta에서 통과 | Mach-O x86_64 실행 파일 |
| `x86_64-unknown-linux-gnu` | Zig 교차 링커로 통과 | Linux에서는 실행하지 않음 | ELF x86-64 실행 파일 |
| `aarch64-unknown-linux-gnu` | 최종 링크 실패 | Linux에서는 실행하지 않음 | Zig 0.16.0이 `--fix-cortex-a53-843419`를 거부함 |

AArch64 Linux Rust 대상은 링커에 `--fix-cortex-a53-843419`를 전달한다.
설치된 Zig 링커는 이 인자를 거부한다. 설치된 LLVM 링커는 인자를 받지만,
LLVM Clang 드라이버에 AArch64 GNU 시작 파일과 라이브러리가 없다. S-4는
진행 중이다. Rust 대상의 링크 인자를 지원하는 AArch64 GNU 링커와
라이브러리를 설치하거나 격리된 AArch64 Linux 빌드 환경에서 전체 빌드를
재시도한다. 링크된 실행 파일을 기록한 뒤 S-4를 완료한다.
