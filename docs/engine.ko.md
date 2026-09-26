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

V8 소스 체크아웃은 버전 150.4.0을 유지하면서 컴파일 시 `paste` 매크로 의존성으로
`pastey 0.2.3`을 선택한다. 이전 매크로 패키지에는 유지보수 종료 권고가 있고 수정
릴리스는 없다. 이 교체는 의존성 패키지만 바꾸며 V8 Rust 매크로 호출이나 사전 빌드
아카이브를 변경하지 않는다. 엔진 검증 워크스페이스는 절대 경로의 로컬 V8 체크아웃을
사용한다. 의존성 변경 후 네 대상의 전체 빌드와 링크를
검증한다.

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

ARM64 Mac에서 `python3 tools/verify_engine.py aarch64-apple-darwin --offline`을 실행하면
로컬 아카이브 해시를 검사하고 Cargo 네트워크 접근을 차단한 상태로 빌드한다. 이 모드는 `var/v8`의
아카이브를 요구하며 다운로드하지 않는다. 도구 사례는 아카이브 누락·변조와 다운로드 실패를 거부한다.

`make verify-engine-linux-arm64`는 SHA-256이
`sha256:5b993f23fb69746405496e76f91e511d96c82b5b23304fd5d20b4a80a8b223ea`인
Rust 1.98.1 Bookworm 이미지로 ARM64 Linux 컨테이너 이미지를 빌드한다.
없는 아카이브를 다운로드하고 해시를 검사한 뒤 저장소를 읽기 전용으로
`/src`에 마운트한다. 호스트의 무시된 `var/engine-cargo`와
`var/engine-target` 디렉터리를 `/cargo`와 `/target`에 쓰기 가능한
바인드 마운트로 연결하고 프로그램을 빌드·실행한다. `make`는 Compose 파일에
절대 호스트 경로를 기록한 무시된 `var/engine-compose.yaml`을 만들어 이후
`containerctl status`에서도 같은 마운트 구성을 읽게 한다. 호스트에서 두 디렉터리의 파일을 직접 확인할 수
있고 컨테이너를 교체해도 내용이 유지된다.
이미지, 컨테이너, 호스트 이름은 허용된 테스트 설치 이름을 사용한다.
네이티브 GNU 링커는 Rust 대상의 링크 인자를 유지한다.
로컬 V8 체크아웃도 Cargo patch의 절대 경로에 읽기 전용으로 마운트한다. 컨테이너는
두 소스 체크아웃을 수정할 수 없다.

## 빌드 증거

macOS와 Linux x86_64 빌드는 Rust 1.98.1과 Zig 0.16.0을 사용하는 macOS
arm64에서 실행했다. Linux AArch64 빌드는 Rust 1.98.1을 사용하는 ARM64
Linux 컨테이너에서 실행했다. 다음 결과는 대상별 전체 빌드·링크와 대상
운영체제에서의 실행을 구분한다.

| 대상 | 전체 빌드·링크 | 실행 | 증거 |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | 통과 | 호스트에서 통과 | Mach-O arm64, SHA-256 `39cd07f5f6a87f16cda29737753b8936244435ff391e5d8a3faff4dc0bdc3ad3` |
| `x86_64-apple-darwin` | 통과 | Rosetta에서 통과 | Mach-O x86_64, SHA-256 `acb98063a9b95ed015a289918ecfd956d26b5085ab9ef6a78d4536a2a769eb1b` |
| `x86_64-unknown-linux-gnu` | Zig 교차 링커로 통과 | Linux에서는 실행하지 않음 | ELF x86-64, SHA-256 `7d6be12196b0baea76a2118948f914eebe807ca4e0b5e0d7c31bad87e938f40a` |
| `aarch64-unknown-linux-gnu` | ARM64 Linux 컨테이너에서 통과 | 같은 컨테이너에서 통과 | ELF AArch64, SHA-256 `c4687edd8619e2e9cb04cf639d86e1b8532485f9c29e08bdfc3ec10866282258` |

네 빌드는 관리 중인 매크로 패키지를 선택하는 V8 150.4.0을 사용했다. 네이티브
AArch64 Linux 빌드는 Rust 1.98.1과 위 아카이브 해시로 완료됐다. 마운트 검사에서
`/src`와 V8 체크아웃은 읽기 전용, `/cargo`와 `/target`은 쓰기 가능한 호스트
마운트로 확인됐다. `containerctl status`는 절대 Compose 경로를 프로젝트 오류 없이
읽었다. 엔진 컨테이너만 교체한 뒤에도 캐시된 소스와 링크된 프로그램이 호스트에
남았다. 반복한 Cargo 빌드는 크레이트 컴파일 없이 0.50초에 완료됐고, 실행 파일은
SHA-256 해시를 유지하며 교체한 컨테이너에서 실행됐다. `make verify-engine-deps`는
권고, 라이선스, 소스 및 금지 의존성 오류를 보고하지 않는다. 이름 있는 볼륨은
필요하지 않다. Linux x86_64는 macOS에서 교차 링커로 전체 링크했다. Linux
x86_64에서의 실행은 이 빌드 기준에 포함되지 않는다.
