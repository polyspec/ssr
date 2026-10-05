[English](engine.md)

# 엔진 검증

## 버전과 로컬 입력

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
이 경로의 SHA-256을 검사하지 않으므로, `tools/verify_archive.py`가 압축된 archive와 일치하는
binding을 검증한 뒤 Cargo를 실행한다. 선택한 대상의 오프라인 잠금 Cargo metadata에서 정확한
V8 버전과 feature 구성을 검사한다. `make check`, `make bench`와 엔진 검증
명령은 소스 빌드 설정을 거부하고 검증한 로컬 입력만 선택한다. Cargo 명령을 직접 실행할 때도 같은
입력 검증을 먼저 수행하고 archive와 binding 환경 변수를 명시해야 한다.

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

`make verify-archive`는 `var/v8`의 로컬 archive와 binding을 검사한다. `make check`와
`make bench`는 연결 전에 이 검사를 실행한다. binding 이름은
`src_binding_simdutf_release_<target>.rs`이고 일치하는 네이티브 feature는 정확히 `simdutf`,
`use_custom_libcxx`다. `default` 표식은 `use_custom_libcxx`를 선택하므로 있어도 되며
표식이 없어도 네이티브 feature 구성은 같다. 파일 누락, 해시 변경, 심볼릭 링크나 상대 경로, Cargo feature 불일치는
실패다. `--archive`, `--binding`, `--metadata-root`는 명시적으로 지정할 수 있다.
`--files-only`는 Cargo 설정이 생성되기 전에 준비한 파일만 검증하며 정상 빌드는 생성된
의존성 그래프도 검증한다.

| 대상 | 생성 binding SHA-256 |
| --- | --- |
| macOS AArch64 및 x86_64 | `ca5adf0cf89c9a70ad460ae73648b2fe89b74aa113b3cb7f757b6a02b758394f` |
| Linux AArch64 및 x86_64 | `7727826ae479bdb645e807239fb12d1f8e2e23de7a6cf16f5ee592690d1d8506` |

`python3 tools/verify_engine.py <target>`는 이미 있는 검증한 입력을 요구하고 Cargo 네트워크
접근 없이 빌드한다. 전체 빌드 경과 시간 제한을 적용하지 않고 Cargo 명령과 컴파일러 진행을
기록한다. 네이티브 대상 실행에는 개별 테스트 제한 시간을 적용한다.

`make verify-engine-linux-arm64`는 SHA-256이
`sha256:5b993f23fb69746405496e76f91e511d96c82b5b23304fd5d20b4a80a8b223ea`인
Rust 1.98.1 Bookworm 이미지로 ARM64 Linux 컨테이너 이미지를 빌드한다.
검증한 로컬 archive와 일치하는 binding을 요구하고 저장소를 읽기 전용으로
`/src`에 마운트한다. 호스트의 무시된 `var/engine-cargo`와
`var/engine-target` 디렉터리를 `/cargo`와 `/target`에 쓰기 가능한
바인드 마운트로 연결하고 프로그램을 빌드·실행한다. `make`는 Compose 파일에
절대 호스트 경로를 기록한 무시된 `var/engine-compose.yaml`을 만들어 이후
`containerctl status`에서도 같은 마운트 구성을 읽게 한다. 호스트에서 두 디렉터리의 파일을 직접 확인할 수
있고 컨테이너를 교체해도 내용이 유지된다.
checkout마다 stack을 따로 둔다. Compose project는 `ssr-engine-<자리>`,
container는 `ssr-engine-<자리>-engine`, image는 `localhost/ssr-engine-verify-<자리>:0.0.1`이며,
자리는 checkout path SHA-256의 앞 16진수 12자리다. `tools/verify_engine_linux.py`는
`make verify-engine-linux-arm64`와 `make verify-engine-down`의 단계를
`tools/holder_lock.py`의 checkout lock `var/locks/engine-verification.lock` 아래에서
실행한다. lock record는 holder의 checkout, pid, process 시작 시각을 밝힌다. 두 번째
실행은 그 record와 함께 거부되고, holder가 더 이상 실행되지 않는 lock은 보고하고
`python3 -m tools.holder_lock remove-stopped <lock file>`이 지울 때까지 남겨 두며, holder만
lock을 해제한다. 각 단계는 command, 출력, 결과를 경과 시간과 함께 시간 제한 없이 출력한다.
네이티브 GNU 링커는 Rust 대상의 링크 인자를 유지한다.
로컬 V8 체크아웃도 Cargo patch의 절대 경로에 읽기 전용으로 마운트한다. 컨테이너는
두 소스 체크아웃을 수정할 수 없다.

## 렌더 스냅샷

렌더 프로세스 하나는 불변 번들 키 하나를 선택한다. 공개 스냅샷 생성기가 context를 한 번
초기화하고 직렬화한 뒤 완전히 정리되면 워커 isolate가 같은 blob을 복원한다. 모든 풀이 종료된
후에도 다른 키는 거부한다. 다른 번들은 별도 프로세스를 사용한다. 요청 context는 독립적이고
어느 번들도 다시 평가하지 않는다.

컴파일러는 기본 V8 스냅샷을 사용하므로 isolate 생성·정리와 렌더 스냅샷 생성을
`ssr_core::process`로 조율한다. 스냅샷 초기화 중이나 성공 뒤에는 컴파일러가 진입할 수 없다.
스냅샷 생성기는 초기화 성공과 실패 모두에서 소비한다. 고정된 V8 구현은 마지막 isolate를
정리할 때 공유 읽기 전용 데이터를 제거한다. 실행 테스트는 초기화 실패, 후속 Svelte 컴파일,
성공한 렌더 순서를 검증한다. 릴리스 빌드가 호환되지 않는 진입을 허용해도 공유 힙의 안전성을
입증하지 못하므로 V8 진입 전에 계약을 검사한다.

macOS AArch64의 공식 archive 네이티브 테스트는 프로세스 키 변경 거부와 같은 초기화 난수를
유지하는 워커 4개의 병렬 렌더 80회, 요청 변경값 격리를 검증한다. React 스트림 테스트는 반복
요청 context에서 실제 `renderToReadableStream` 출력을 검증한다. 다중 프로세스 테스트는
애플리케이션 번들 3개와 모듈 하나를 별도 프로세스에서 동시에 렌더한다. 이 테스트는 다른 대상의
빌드 증거와 별개다.

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
