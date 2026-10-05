# 체크리스트

## 항목

- [o] S-0 작성자 `min-median-max`로 로컬 Git 저장소를 초기화하고 `AGENTS.md`, 이 체크리스트, 변경 기록,
  `.gitignore`를 둔다. 의존: 없음. 증거: 첫 커밋, `git remote -v` 출력 없음.
- [o] S-0-1 ssr의 렌더·빌드·엔진 요구 사항과 검증 항목을 명세한다. 의존: S-0. 증거:
  [요구 사항](requirements.ko.md)의 요구 사항과 S-3·S-4의 빌드·엔진 완료 기준이 커밋됨.
- [o] S-0-2 절차와 체크리스트의 두 언어 파일에 일시 우회 상태를 정의하고 체크리스트 상태 검사에 포함한다.
  의존: S-0-1. 증거: 문서 검사가 두 언어 파일을 검증하고 이 변경이 커밋됨.
- [o] S-1 [AGENTS.ko.md](../AGENTS.ko.md)의 크레이트로 Cargo 워크스페이스(`rust-toolchain.toml`: Rust 1.98.1,
  edition 2024)와 Makefile 목표 `check`(rustfmt, clippy `-D warnings`, `cargo deny check`, 기록·용어·문서 검사,
  테스트별 타임아웃을 가진 cargo-nextest 0.9.146 단위 테스트), `bench`를 만든다. 의존: S-0-1. 증거: `make check`
  종료 코드 0. 완료 기준: 매니페스트에는 AGENTS.md의 여덟 크레이트만 있고 모든 크레이트가 edition 2024로
  컴파일된다. `check`는 Rust 형식 오류, Clippy 경고, 의존성 발견 사항, 부적절한 기록·용어, 문서 쌍·링크 오류,
  실패하거나 시간 초과한 테스트에서 실패한다. `bench`는 실행할 수 있는 목표다.
- [o] S-1-1 문서·용어 검사의 범위를 관리되는 소스로 제한한다. `node_modules`의 설치된 패키지 파일은 의존성이며
  제품 기록으로 검사하지 않는다. 의존: S-1. 증거: 검사기가 관리되는 문서의 오류를 보고하고 중첩된
  `node_modules`의 패키지 README는 무시하며, 패키지 설치 후 `make check` 종료 코드가 0이다.
- [o] S-1-2 의존성 검사에서 지정된 라이선스를 허용한다. MIT, Apache-2.0, BSD-2-Clause,
  BSD-3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0 밖의 라이선스는 정확한 크레이트 예외가
  기록되지 않았다면 오류다. 의존: S-1.
  증거: `make check` 종료 코드 0, cargo-deny의 라이선스 검사 유지.
- [o] S-1-3 필요한 빌드 의존성의 라이선스를 검토하고 `deny.toml`에 두 크레이트의 정확한 버전만
  허용한다. 다른 크레이트와 버전에는 공통 라이선스 정책을 유지한다. 의존: S-1-2.
  증거: 지정된 버전의 라이선스 검사는 통과하고 다른 버전은 실패하며, `make check` 종료 코드 0.
- [o] S-1-4 워크스페이스의 테스트 소유 위치를 강제한다. 선언한 각 공유 동작의 기본 테스트를 소유
  크레이트에서 실행하고 모든 소비 크레이트에서 실제 사용 테스트를 실행한다. 누락·잘못된 위치·빈 테스트·
  실패·시간 초과와 파일 이름만 적은 선언은 거부한다. 의존: S-1. 완료 기준: 선언은 공유 페이지·빌드
  동작과 각 소유 크레이트의 모든 직접 워크스페이스 소비자를 포함하며, 각 사례는 해당 크레이트의 통합
  테스트 대상과 정확한 테스트 함수 하나를 지정한다. 증거: 각 잘못된 선언의 변형 사례가 실패하고
  선언한 테스트가 각 크레이트에서 실행되며 `make check`가 소유 위치 검사를 실행한다.
- [o] S-2 `ssr-core`를 구현한다: 렌더 모드 `ssr` 또는 `csr`을 가진 `POST /_render`의 렌더 호출
  `Page{render,title,language,props,state}`, 결과(HTML과 출력 상태), 오류. 캐시 헤더는 이 크레이트에
  포함하지 않는다. 의존: S-1. 증거: JSON fixture가 ordered-json으로 왕복. 완료 기준: 필수 필드가
  정확히 다섯 개이고 `render`는 `ssr` 또는 `csr`, `title`과 `language`는 문자열, `props`는 객체,
  `state`는 모든 JSON 값이다. 필드 누락·추가, 잘못된 타입과 JSON은 오류를 반환한다.
  [렌더 계약](core.ko.md)을 따른다.
- [o] S-2-1 페이지 JSON 요청의 모든 깊이에서 해석된 객체 키가 반복되면 거부한다.
  같은 키를 만드는 이스케이프 표기도 포함한다. 의존: S-2. 증거: 추적되는 중복 키
  사례가 ordered-json 파싱에서 실패하고 고유 키 fixture는 왕복하며 `make check` 종료 코드가 0이다.
- [o] S-3 번들러를 검증한다: rolldown 1.2.11의 Rust API로 React TSX에서 코드 분할과 내용 해시 이름을 가진 서버
  번들과 클라이언트 번들을 만들고, lightningcss 1.0.0-alpha.72로 `node_modules`의 `@import`와 `url()` 재작성을
  포함한 CSS를 묶고, 글꼴과 이미지(png, svg, jpg, gif, webp, avif, ico, woff, woff2, ttf)를 해시 파일로 산출한다.
  결과와 선택한 API를 `docs/build.md`에 기록하며, 지원되지 않는 경우는 S-5 시작 전에 대체 방법과 함께 적는다.
  의존: S-1. 증거: 경우별 샘플 빌드와 문서. 완료하려면 지원되는 CSS 번들러 의존성과
  `make check` 통과가 필요하다.
- [o] S-4 엔진을 검증한다: deno_core 0.412.0을 deno_webidl 0.259.0, deno_web 0.290.0과 함께 aarch64·x86_64
  macOS와 Linux에서 빌드하고, `deno_v8` 0.4.0이 prebuilt 아카이브를 얻는 방법, 로컬 아카이브를 고르는 변수, 그
  SHA-256을 기록한다. 의존: S-1. 증거: 빌드와 `docs/engine.md`.
- [o] S-4-1 엔진 검증 소스는 읽기 전용으로, Cargo 홈과 대상 디렉터리는 무시된 `var/` 아래 쓰기 가능한 호스트
  디렉터리로 마운트한다. 호스트에서 파일을 직접 확인할 수 있고 엔진 컨테이너를 교체해도 다운로드한 크레이트와 빌드
  결과를 유지한다. 의존: S-4. 완료 기준: 세 마운트의 접근 권한이 요구대로이고, `containerctl status`가 절대 경로
  구성을 읽으며, 컨테이너 교체 후 두 번째 빌드에서 크레이트를 다시 컴파일하지 않고 링크된 프로그램을 재사용하고,
  named volume을 만들지 않는다. 증거: 마운트·상태 검사, 두 번의 빌드 및 `docs/engine.md`에 기록된 호스트 파일 검사.
- [o] S-4-2 V8 소스 체크아웃에서 유지보수가 종료된 컴파일 시 식별자 매크로 의존성을 관리 중인 패키지로
  교체하고 엔진 검증 빌드에 해당 체크아웃을 사용한다. 의존: S-4-1. 완료 기준: `cargo deny check`가
  유지보수 종료 매크로 의존성을 보고하지 않고, 엔진 검증 프로그램이 macOS와 Linux의 AArch64·x86_64에서
  전체 빌드와 링크를 완료하며, Linux AArch64 컨테이너는 V8 체크아웃을 읽기 전용으로 마운트한다.
  증거: V8 패키지의 로컬 커밋, 네 대상 빌드, 마운트 검사와 `docs/engine.ko.md`.
- [o] S-5 S-3의 결정대로 `ssr-build`를 구현한다: 서버와 클라이언트 번들, CSS, 해시 자산, 매니페스트. 의존: S-2,
  S-3. 완료 기준: 절대 소스 경로와 로컬 절대 자산 경로를 받아 생성된 전체 바이트와 ordered-json 매니페스트를
  반환하며, 진입점·import·자산 누락, 잘못된 자산 경로, 다른 바이트를 가진 동일 출력 경로에서 실패한다. 매니페스트는 서버 진입점,
  클라이언트 진입점, 클라이언트 스타일, 서버 청크와 공개 자산을 SHA-256과 함께 식별한다. 증거: 샘플
  애플리케이션 빌드, 매니페스트와 바이트의 일치, 두 빌드의 해시와 출력 바이트 동일.
- [o] S-5-1 React 번들의 CSS import를 명시적으로 처리하고 애플리케이션 빌드 경로를 테스트한다.

- [o] S-6 `ssr-runtime`을 구현한다: 워커 스레드마다 isolate 하나와 제한된 대기열을 가진 isolate 풀, isolate마다
  번들 한 번 컴파일, `console`과 운영체제의 `crypto.getRandomValues` 자체 연산, fetch·I/O 타이머·파일·네트워크
  없음, 실행 중인 스크립트를 종료하는 타임아웃, 요청 사이 전역 상태 초기화. 의존: S-2, S-4, S-4-2. 증거: 요청 A의 전역
  변경이 요청 B에 없음, 끝나지 않는 스크립트가 타임아웃에 종료됨, 가득 찬 대기열이 오류를 반환, Web API 사례 통과.
- [o] S-6-1 React 서버 번들이 실행되도록 모든 렌더 컨텍스트에 UTF-8 `TextEncoder` 인터페이스를 제공한다.
  의존: S-6. 완료 기준: 생성, `encoding`, `encode`, `encodeInto`가 ASCII, 비ASCII 문자, 단독 서로게이트와
  짧은 대상 배열에 대한 UTF-8 동작을 따르고 잘못된 수신자와 대상은 실패한다. 컨텍스트 초기화 후에도 사용할 수
  있으며 파일·네트워크·타이머 API를 추가하지 않는다. 증거: Web API 테스트와 React 서버 번들 실행.
- [o] S-6-2 요청이 제한된 대기열에 들어간 것을 확인한 뒤, 하나의 렌더 기한이 풀 대기와 실행을 모두
  포함하는지 검증한다. 대기 수에는 이벤트가 없으므로 사례는 제한 시간을 둔 yield 반복으로 이를 관찰한다.
  의존: S-6. 증거: 동기화된 시간 초과 사례와 `make check` 통과.
- [o] S-6-3 스트림 종료 또는 설정한 시간 만료 시 요청을 취소하고 워커 정리가 끝난 뒤에만
  용량을 반환한다. 우선순위: P0. 의존: S-6, S-13-2. 완료 기준: 중단된 수신자가 네이티브 청크
  전송을 무기한 막지 않고 이전 요청의 늦은 취소가 다음 요청을 종료하지 않는다. 풀 상태와 종료는
  응답하지 않는 워커를 보고한다. 워커나 관측 스레드의 unwind는 추가 렌더 호출 없이
  `Pool::wait`에 통지한다. `PoolOptions`에 입력·대기 바이트·출력·청크·힙 한도를 명시하고
  `Cancellation`은 요청 하나를 식별한다. 큰 V8 청크는 출력 바이트를 바꾸지 않고 한도 이하
  조각으로 나눈다. HTTP 입력 한도 초과는 413, 대기열 한도 초과는 503을 반환한다. 근거: 추적되는
  취소·시간 만료·역압·용량·종료·바이트 한도 사례가 수정 전에 실패하고 수정 후 통과하며, 소유
  라이브러리와 소비자의 스트림 사례 및 `make check`가 통과한다.
- [o] S-7 React 어댑터를 구현한다: SSR은 렌더 상태 출력과 함께 HTML을 렌더하고, CSR은 같은
  클라이언트 번들의 정적 셸을 반환하며, 클라이언트는 SSR HTML을 hydrate하거나 CSR 셸을 렌더한다. 의존: S-5, S-6, S-6-1.
  완료 기준: 어댑터는 하나의 애플리케이션 컴포넌트를 위한 서버·클라이언트 React 진입점을 제공하고 두 문서에서
  같은 클라이언트 URL을 사용한다. SSR은 입력 상태를 컴포넌트에 전달하고 출력 상태를 반환하고 문서에 담는다.
  CSR은 root를 비워 두고 입력 상태를 변경하지 않고 담으며 런타임 풀을 사용하지 않는다. 정적 CSR 셸은 빈 props와
  null 상태를 담고 서로 다른 요청 경로에 재사용할 수 있다. HTML의 JSON은 script 요소를 닫을 수 없으며 잘못된
  로컬 클라이언트·스타일 URL과 정적 셸 값은 오류다. 증거: [React 어댑터](react.ko.md)에 계약을 기록하고,
  실제 Chrome 브라우저에서 SSR hydration, CSR 렌더, script 입력과 두 경로의 정적 셸 재사용을 검증한다.
- [o] S-8 게시와 제공을 구현한다: 빌드의 공개 파일을 디렉터리에 원자적으로 쓰고, 같은 내용의 기존 파일은 유지하며,
  다른 내용의 기존 파일은 오류다. 빌드 파일을 콘텐츠 타입과 함께 제공한다. 의존: S-5. 완료 기준: 공개 URL이
  있는 매니페스트 항목만 선택하고 바이트와 SHA-256 값이 일치해야 하며 URL 경로는 안전하고 중복되지 않아야 한다.
  게시는 이미 존재하는 절대 디렉터리를 요구하고 같은 내용의 파일과 관련 없는 파일을 유지하며 다른 바이트와 일반
  파일이 아닌 대상을 거부한다. 새 파일은 전체 내용을 기록한 뒤에만 보이게 한다. 제공은 정확한 공개 URL의
  GET과 HEAD를 처리하여 매니페스트의 콘텐츠 타입과 바이트를 반환하고, 알 수 없는 경로에는 404를, 다른
  메서드에는 405를 반환한다. 증거: 게시 세 사례, 잘못된 입력 사례와 HTTP 제공 사례.
- [o] S-8-1 완전하고 변경되지 않는 빌드 디렉터리를 저장하고 읽는다. 우선순위: 준비된 빌드 파일로
  렌더러를 시작하기 전에 필요하다. 의존: S-5, S-8. 완료 기준: `Build::write`는 결정적인 SHA-256
  이름의 디렉터리 아래 매니페스트와 모든 비공개·공개 파일을 원자적으로 게시하며 같은 쓰기는 해당
  디렉터리를 유지한다. `Build::read`는 누락·변조·추가·중복·안전하지 않은 경로·심볼릭 링크와 잘못된
  매니페스트 필드를 거부한다. 게시 실패는 이전 빌드를 보존하고 정리 실패도 보고한다. 증거: 추적되는
  RED/GREEN 저장·무결성·동시 게시·공개 파일 선택 사례와 `make check`.
- [o] S-9 `ssr-server`를 구현한다: `POST /_render`, 렌더 시간·풀 대기·힙의 tracing 메트릭, sourcemap 9.3.2로
  역매핑한 스택. 의존: S-7, S-8. 완료 기준: 빌드는 서버 진입점과 서버 청크의 비공개 소스맵을 일정한
  바이트와 전체 SHA-256으로 기록한다. 런타임은 측정한 풀 대기 시간과 현재 V8 힙 바이트를 반환하고,
  서버는 SSR의 이 값과 렌더 시간을 tracing에 기록한다. 엔드포인트는 잘못된 메서드·콘텐츠 타입·질의·페이지를
  거부하고 UTF-8 JSON 콘텐츠 타입을 허용하며 SSR과 CSR의 HTML 문서를 반환한다. 렌더 실패는 명시적
  오류로 반환한다. 서버 스크립트 이름의 JavaScript 스택 프레임을 소스 위치로 역매핑하고,
  소스맵이 없거나 잘못되면 서버 생성이 실패한다.
  [HTTP 서버 계약](server.ko.md)을 따른다. 증거: 일정한 빌드 출력, 런타임 메트릭, HTTP 사례,
  역매핑 스택과 `make check`.
- [o] S-9-1 개발 빌드와 렌더 서버를 별도 Rust 프로세스에서 실행하며 준비·종료·재시작 횟수·이벤트 용량·검증 본문 바이트의 `ProcessOptions`를 명시한다. 우선순위: 렌더 프로세스의 불변 스냅샷에 필수. 의존: S-9, S-14, S-8-1. 완료 기준: 빌드 명령은 검증된 불변 빌드 디렉터리를 생성하며 렌더 명령은 해당 디렉터리를 제공하고 실제 SSR 요청을 완료한 뒤 트래픽을 받는다. 교체는 이전 프로세스를 사용하는 응답을 유지하고 해당 응답 종료 후 이전 프로세스를 중지하고 종료를 확인한다. 빌드 또는 준비 실패 시 새 요청은 명시적으로 실패한다. 예상하지 않은 렌더 프로세스 종료를 폴링 없이 관측하고 같은 완료 빌드에서 대체 프로세스를 시작한다. 종료는 자식 프로세스를 중지하고 종료를 확인하며 강제 종료를 보고한다. 증거: 추적되는 프로세스·준비·재빌드 실패·스트림 종료·프로세스 종료·재시작·정리 테스트, `make check` 통과.
- [o] S-9-1-1 감독되는 렌더러 요청에 비공개 Unix 소켓을 사용한다. 우선순위: 서비스 DNS 없이 프로세스 준비를 완료하는 데 필요하다. 의존: S-9-1. 감독 프로세스가 자식을 실행하기 전에 소켓 디렉터리를 생성하고 소유하며 명령 생성 함수에 정확한 소켓 경로를 전달한다. 다른 선언 경로는 거부하고 프로세스 종료를 수집한 뒤 자신이 소유한 디렉터리만 제거한다. 수용 기준: 추적 RED·GREEN이 절대 비공개 소켓, 요청·응답 스트림, 준비·교체·배출·취소·종료 및 소켓 정리를 검증한다. 준비는 네트워크 주소, 상대 경로나 다른 선언 경로, 소켓이 아닌 선언 경로, 다른 사용자에게 열린 소켓 디렉터리, 다른 항목이 있는 소켓 디렉터리를 거부하고, 감독 프로세스는 그 항목을 남긴다. Unix 소켓 주소보다 긴 소켓 경로는 디렉터리를 만들기 전에 거부한다. `crates/ssr-server/tests/development.rs`의 재시작 사례는 공개 `Development` API로 실제 자식을 소켓 위에서 실행하고, 프로세스 종료 뒤 모든 소켓 디렉터리가 제거되었음을 요구한다. Red: 경로 길이 사례는 `Socket::create`가 디렉터리를 만들고 오류를 반환하지 않아 실패했다. 소켓이 아닌 경로와 열린 디렉터리 사례는 둘 다 `render socket requires a private directory`를 반환해 실패했다. 나머지 새 사례는 첫 실행에서 통과했고, 각각 commit하지 않은 mutation에 대해 실패했다. 경로 비교를 없애면 네트워크 주소, 상대 경로, 다른 경로 사례가 `render socket path must be absolute and canonical`이나 준비 시간 초과로 실패했다. `remove_dir_all`을 쓰면 다른 항목 사례가 `the supervisor removed a file it did not create`로 실패했다. 디렉터리 mode를 0755로 하면 절대 소켓 사례가 실패했다. 프로세스 종료 뒤 소켓 제거를 없애면 development 사례 7개가 `socket directory remains`로 실패했고, 준비 실패 뒤 제거를 없애면 준비 사례가 실패했다. 응답 본문을 버리지 않으면 취소 사례가 `shutdown timed out; forced termination`으로 실패했다. Green: `ssr-server` 단위 사례 19개와 `process` 사례가 통과하고, `development` 사례 16개 중 15개가 통과한다. 나머지 사례는 소스 감시 때문에 main에서도 간헐적으로 실패하며 S-9-1-2다. `cargo clippy -p ssr-server --all-targets -- -D warnings`와 `cargo fmt --all -- --check`가 통과한다.
- [o] S-9-1-2 `Development::start`가 반환된 직후 쓴 소스 변경을 관측한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 의존: S-9-1. 원인: macOS의 소스 감시는 FSEvents를 썼고, 그 daemon은 event를 늦게, 전달 보장 없이 보낸다. 감시 직전에 쓴 file의 event가 `start` 뒤에 도착해 두 번째 빌드를 시작했고, host load average 30~135에서 event가 쓰기 뒤 최대 89 s에 도착했으며, stream이 감시 뒤에 쓴 file의 event를 전달한 뒤에 한 쓰기가 14.5분 동안 event를 만들지 않았다. 그래서 시작 뒤 소스 변경을 쓰는 development 사례가 간헐적으로 실패했다. 수용 기준: `Development::start`는 소스 경로 다음에 비공개 디렉터리를 감시하고 그 안에 sentinel file을 쓴 뒤, sentinel event가 도착한 뒤에만 첫 빌드를 시작한다. 그 전의 event는 빌드를 시작하지 않는다. 이 대기는 시간 제한이 없고 시작과 끝을 기록한다. macOS에서 소스 감시는 제외 경로를 건너뛰는 walk로 감시하는 모든 디렉터리와 일반 file을 `kqueue(2)`에 등록한다. 디렉터리가 바뀌면 새 항목을 생성 알림 전에 등록하고 없어진 항목의 등록을 해제한다. 등록 전에 walk가 필요한 file descriptor 수를 세어 soft descriptor limit을 그 수만큼 또는 hard limit까지 올리고, 들어가지 않으면 그 수, limit, 가장 큰 디렉터리 다섯 개를 적어 실패한다. Linux는 `inotify(7)`를 유지한다. 소스 감시는 내부 seam이어서 test가 event 도착 시점을 정하고, `docs/server.md`가 descriptor 비용을 적는다. development 사례는 event로 판정한다. `change`의 15 s 제한과 `failed_rebuild_starts_the_previous_process_shutdown_duration`의 3 s 제한을 없앤다. Red: 아직 등록되지 않은 경로의 event를 잃는 fake watch로 `start_builds_once_from_the_watch_baseline`이 5회 중 5회 `start returned without a watch baseline after 1 builds; the event of a write made before the watch was forwarded`로 실패했다. 등록되지 않은 경로의 변경을 잃는 fake kernel의 `kqueue` 사례는 새 항목을 등록하지 않고 알리는 mutation에 대해 `the creation is reported only after the file is registered`로 실패했고, 중첩 디렉터리 사례는 새 디렉터리만 알렸다. Green: start 사례가 5회 중 5회 통과하며 sentinel event 뒤에만 한 번 빌드하고 그 뒤의 쓰기에 한 번 더 빌드한다. `kqueue` 사례 7개가 통과한다. 디렉터리 event 직후 만든 file에 쓴 내용은 fake kernel이 잃지만 생성이 시작한 빌드가 읽으며, 중첩된 새 디렉터리, 제외 경로, 없어진 항목, 비재귀 디렉터리, descriptor limit 결정, 가장 큰 디렉터리를 검증한다. `ssr-server` 단위 사례 29개가 통과하고, `development`와 `process` 사례 16개가 `kqueue` backend에서 3회 중 3회 통과한다. 실제 backend에서 host load average 약 18일 때 `start` 직후 소스 변경을 쓴 30회는 30회 모두 시작 시 한 번 빌드했고, 쓰기 뒤 83~214 ms에 변경을 렌더했으며, sentinel event는 5 ms 안에 도착했다. 측정을 위해 만들지 않은 host의 다른 작업으로 load average가 66~70일 때 같은 30회는 30회 모두 시작 시 한 번 빌드했고 쓰기 뒤 최대 283 ms에 변경을 렌더했으며 sentinel event는 2 ms 안에 도착했다. `cargo clippy -p ssr-server --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo deny check`가 통과한다. Linux target이 없는 이 host에서 `inotify` 경로의 Linux 빌드는 컴파일하지 않았고, 전체 묶음이 컴파일한다.
- [o] S-9-1-2-1 Linux 소스 감시를 컴파일한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 의존: S-9-1-2. 원인: S-9-1-2는 macOS가 아닌 target의 `inotify` 소스 감시를 `notify`라는 module에 두었다. 이 module이 `notify` crate를 가려 `ssr-server`가 Linux에서 컴파일되지 않았고, Linux 경로를 컴파일한 빌드가 없었다. 수용 기준: 모든 target을 포함한 `ssr-server`의 `cargo check`와 `cargo clippy -- -D warnings`가 `aarch64-unknown-linux-gnu`와 `x86_64-unknown-linux-gnu`에서 통과하고 macOS 빌드도 계속 통과한다. Red: `cargo check --target aarch64-unknown-linux-gnu -p ssr-server --all-targets`가 `unresolved import notify::Event`, `cannot find type Result in module notify`, `enum import RecursiveMode is private`로 실패했다. Green: module 이름을 `inotify`로, crate를 `::notify`로 쓰자 두 Linux target에서 check와 clippy가 통과하고 macOS clippy도 통과한다. Linux clippy는 handler를 `recommended_watcher`에 바로 넘기도록 요구했다. C 의존성은 저장소 밖의 wrapper로 `zig cc`를 써서 Linux target으로 컴파일했다. S-22가 Linux 검사를 전체 묶음에 추가한다.
- [o] S-9-1-3 소스 감시가 버린 event를 알리면 다시 빌드한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 의존: S-9-1. 원인: file event 서비스가 event를 버리면 `notify`는 `Rescan` flag가 있는 `Other` 종류의 event를 알린다. `source_event`는 생성, 수정, 삭제, any event만 받으므로 감독 프로세스는 그 알림을 버리고, 버려진 변경은 빌드에 이르지 못한다. 수용 기준: 소스 경로의 `Rescan` event는 다시 빌드를 시작한다. 제외 경로는 여전히 빌드를 시작하지 않는다. 감독 프로세스는 이런 재빌드에 경고를 기록한다. Red: `dropped_event_reports_rebuild_unless_only_excluded`가 `assertion failed: source_event(&rescan, &excluded)`로 실패했다. Green: 소스 root의 `Rescan` event, 경로 없는 `Rescan` event(inotify queue overflow), 재빌드하지 않는 제외 경로의 `Rescan` event로 사례가 통과하고, `ssr-server` 단위 사례 21개, `cargo clippy -p ssr-server --all-targets -- -D warnings`, `cargo fmt --all -- --check`가 통과한다.
- [o] S-9-1-4 빌드가 실행되는 동안 도착한 소스 event에 대해 한 번만 빌드한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 의존: S-9-1-2. 원인: `notify`와 `kqueue` 감시는 file 쓰기 한 번을 여러 event로 알리고 감독 프로세스는 event마다 차례로 다시 빌드했으므로, 쓰기 한 번에 애플리케이션을 여러 번 빌드하고 제공되는 빌드가 여러 번 바뀌었다. 증거: 새 file에 대한 `fs::write` 한 번이 FSEvents backend에서 같은 경로의 `Create(File)`, `Modify(Metadata(Extended))`, `Modify(Data(Content))` event를 만들었다. 수용 기준: 감독 프로세스는 소스 event를 하나 받으면 이미 쌓인 event를 모두 함께 받아 그 모두에 대해 다시 빌드를 한 번 시작한다. 그 빌드가 실행되는 동안 도착한 event는 그 뒤에 정확히 한 번 더 빌드를 시작하고, 이것이 반복되므로 버려지는 event가 없고 각 빌드 뒤에는 많아야 한 번의 빌드가 따른다. 시간 기반 debounce는 쓰지 않는다. kernel은 저장 한 번의 변경을 두 번에 나누어 알릴 수 있고 두 번째가 빌드 시작 뒤에 오면, 그 저장은 빌드 중에 도착한 event의 후속 빌드로 두 번째 빌드를 만든다. Red: `events_during_a_build_start_one_follow_up_build`는 각 `development_process` 빌드를 FIFO로 붙잡고, 첫 빌드 동안 event 세 개, 후속 빌드 동안 event 하나를 보내며, 3회 중 3회 `left: 6, right: 4` 빌드로 실패했다. Green: 사례가 3회 중 3회 빌드 4번으로 통과한다. 첫 event에 한 번, 세 event에 대한 후속 한 번, 그 동안의 event에 대한 후속 한 번, 그 뒤의 쓰기에 한 번이며 그 쓰기도 관측된다. 실제 `kqueue` backend에서 host load average 약 22일 때 `start` 뒤 저장 20번은 19번이 빌드 한 번, 한 번이 빌드 두 번을 만들었고, 감독 프로세스는 event 3개에 17번, 2개에 3번, 1개에 3번 다시 빌드했다고 기록했다. `ssr-server` 단위, `development`, `process` 사례 46개가 통과하고, `cargo clippy -p ssr-server --all-targets -- -D warnings`가 macOS, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`에서 통과한다.
- [o] S-10 서버 번들 해시, deno_core 버전, 라이브러리 버전을 키로 하는 스냅샷을 구현한다. 컨텍스트 초기화는
  스냅샷에서 복원하고, 키가 다르면 새 스냅샷을 만든다. 의존: S-6. 수용 기준: 스냅샷에 초기화한 서버 전역과
  Web API가 포함된다. 모든 요청은 번들을 재실행하지 않고 스냅샷에서 별도 컨텍스트를 복원한다. 요청에서 바꾼
  값은 후속 요청에 전달되지 않는다. 같은 키는 스냅샷 바이트를 재사용하고 번들 또는 버전이 달라지면 새
  스냅샷을 만든다. 스냅샷 오류는 오류로 반환하고 설정한 제한 시간을 초기화에 적용한다.
  증거: 불일치·초기화·격리 테스트 및 `make check`.
- [o] S-11 `make bench`를 구현한다: 고정 페이지의 초당 렌더 수와 동시 호출 1, 4, 16에서의 p50/p99 지연, 컨텍스트
  초기화의 시간과 메모리를 측정하고 한도를 `docs/benchmarks.md`에 기록한다. 의존: S-9, S-10. 완료 기준: 고정 SSR
  페이지를 스냅샷 런타임으로 렌더하고 호출마다 별도의 경과 시간을 측정한다. 모든 호출을 보고하거나 실패하며,
  성공한 모든 호출에서 최근접 순위 p50/p99와 처리량을 계산하고, 컨텍스트 생성 시간과 생성 중 현재 V8 힙
  바이트의 변화를 측정한다. 잘못된 표본 수, 렌더 오류, 기록한 한도를 벗어난 측정값을 거부한다. 증거: 실패·백분위
  사례, 실제 측정 출력과 기록한 한도.
- [o] S-12 S-4가 기록한 대로 V8 아카이브를 경로와 SHA-256으로 고정한다. 다운로드 실패는 오류다.
  명시적인 오프라인 모드는 기존 검증 아카이브를 요구하고 Cargo 네트워크 접근을 차단한다. 의존: S-4.
  증거: 누락·변조·다운로드 실패 사례와 네트워크 없는 빌드 통과.
- [o] S-13 `renderToReadableStream`으로 React 출력을 스트리밍한다: 셸이 준비될 때 상태와 헤더를 확정하고, 셸 전
  렌더 또는 JavaScript 오류는 오류 페이지와 함께 500으로 응답한다. 풀 고갈이나 워커 사용 불가에는 503,
  렌더 시간 초과에는 504를 반환한다. 셸 이후 Suspense 경계 안에서 오류가 나면 fallback과 React의
  클라이언트 복구 지시를 스트림에 보존하고 요청 nonce와 함께 구조화된 서버 오류 이벤트로 `onError`를 기록한다.
  클라이언트 재시도도 실패하면 애플리케이션의
  오류 경계가 오류를 처리한다. 모든 인라인
  script와 style은 호출의 nonce를 가진다. 서버는 요청마다 운영체제 난수 바이트로 nonce 하나를 생성한다.
  난수 실패는 오류이고, 한 호출의 모든 인라인 script와 style은 그 nonce를 사용하며, 다른 호출은 다른 nonce를
  사용한다. React의 스트림과 작업 예약 기능은 프레임워크 스코프에만 제공하고 애플리케이션 코드는 I/O 타이머에
  접근할 수 없다. 의존: S-7, S-13-1, S-13-2. 증거: 셸·늦은 오류·nonce·타이머 격리·SSR 지표 사례 통과.
- [o] S-13-1 Promise 렌더 결과의 V8 microtask를 완료하고, 거부 상세 정보를 반환하며, 예정된 완료가 없는
  Promise를 거부한다. React가 애플리케이션 컴포넌트를 렌더하면서 프레임워크 타이머를
  해당 애플리케이션에 노출하지 않는지 확인한다. 의존: S-6, S-7. 증거: 이행·거부·대기 Promise 테스트,
  React 컴포넌트 테스트 및 `make check` 통과.
- [o] S-13-2 격리된 React 작업 예약과 Web Streams를 제공하고, 셸 준비 후 V8 워커의 스트림 청크를 HTTP
  응답으로 전달하며, 요청 nonce를 인라인 script와 style에 적용한다. 의존: S-13-1, S-13-2-1,
  S-13-2-2, S-13-2-2-3. 증거: 점진적 청크와 확정된 상태·헤더를 포함한 S-13 사례 통과.
  첫 본문 청크 뒤 reader가 거부되면 초기 응답 상태·헤더를 유지하면서 명시적 본문 오류를 반환한다.
- [o] S-13-2-1 별도의 비공개 React 프레임워크 번들과 동일한 React 인스턴스를 import하는
  애플리케이션 서버 번들을 빌드한다. 절대 프레임워크 진입점, 매니페스트 바이트와 SHA-256, 소스맵 및 공개
  URL 부재를 검증한다. 의존: S-13-1. 증거: `useId`를 쓰는 애플리케이션 컴포넌트가 애플리케이션 타이머
  없이 렌더되고, 누락·중복 진입점은 실패하며, 빌드 테스트와 `make check` 통과.
- [o] S-13-2-1-1 실행 가능한 V8 fixture로 React 번들 실행 경계를 정의하고 검증한다.
  fixture는 프레임워크와 애플리케이션 번들을 하나의 context에서 평가해 모듈 초기화에서
  `Component`와 `createContext`를 사용하고, 작업 예약을 함수의 어휘 인자로 전달하며,
  애플리케이션 평가 전에 전역 타이머를 제거한다. 의존: S-13-2-1. 증거: 최상위 React 클래스와
  context가 `useId`, `useContext`로 렌더되고, 애플리케이션 코드에서 `setTimeout`에 접근할 수 없으며,
  `make check` 통과.
- [o] S-13-2-2 격리된 작업 예약과 Web Streams를 가진 두 번들을 실행하고, S-13의 nonce와 오류 동작을
  적용해 HTTP로 문서를 스트리밍한다. 검증된 실행 경계를 실제 스냅샷과 풀에 적용하고 요청마다 번들을
  다시 실행하지 않는다. 반복 복원에서 초기화는 한 번이며 전역 상태가 분리되고, 세 애플리케이션
  스냅샷을 동시에 복원한다. React 애플리케이션 서버 번들은 서버 청크 없는 IIFE이며,
  React 매니페스트가 서버 청크를 기록하면 오류다. vanilla HTTP 경로는 ECMAScript 모듈 청크와
  소스맵 사례를 유지한다. 의존: S-13-2-1-1, S-13-2-2-1, S-13-2-2-1-1, S-13-2-2-2, S-15-1.
  증거: 런타임·서버·브라우저에서 셸, Suspense fallback, 클라이언트 복구, HTTP 500/503/504,
  nonce, SSR 지표, 병렬 복원 사례 통과.
- [o] S-13-2-2-3 React 어댑터와 런타임 문서에 공개 프레임워크·애플리케이션 진입점,
  `Pool::new_react`, `render_stream`, `stream_parts`, CSR 계약을 정확히 기록하고 이전 동기 React
  렌더 설명을 제거한다. 이 수정은 S-13 완료보다 먼저 수행한다. 의존: S-13-2-2.
  증거: 추적되는 계약 검사가 이전 React 문서에서 실패하고 수정한 영·한 문서에서 통과하며,
  공개 React 소비 테스트와 `make check`가 통과한다.
- [o] S-13-3 요청 nonce 문서 재작성기를 재사용 가능한 `ssr-nonce` 크레이트로 공개하고 서버가
  내부 복제본 대신 사용하게 한다. 크레이트는 하나의 요청 nonce를 스트림·완결 문서의 모든
  `script`와 `style` 요소에 적용하고 raw text와 주석을 보존하며, 요소가 다른 nonce를 이미 가지거나
  문서가 UTF-8이 아니면 실패한다. 의존: S-13-2-2. 증거: 크레이트의 분할 태그·raw text·주석·
  nonce 불일치·UTF-8 사례가 통과하고, 서버가 크레이트에 위임하며 문서 경계 사례가 통과하고,
  `make check`가 통과한다.
- [o] S-13-2-2-1 서버 스냅샷마다 V8 isolate group을 만들고 모든 워커 isolate를 해당 group에서 복원한다.
  서로 다른 애플리케이션 스냅샷을 워커가 동시에 복원해도 독립성을 유지하며 워커 isolate 생성을
  직렬화하지 않는다.
  고정된 V8 소스를 release 설정으로 빌드하며 별도 포인터 cage와 외부 코드 공간을 사용한다. 해당 기능을 사용할 수 없으면
  실패한다. 의존: S-10. 증거: 서로 다른 스냅샷 세 개를 여러 차례 동시에 복원하고, 기존 스냅샷·벤치마크
  사례와 `make check`가 통과한다.
- [o] S-13-2-2-1-1 `make check`와 `make bench`가 연결하기 전에 별도 절대 로컬 V8 소스 archive와
  생성된 binding의 기록된 SHA-256을 검증한다. 파일이 없거나 바뀌었거나 소스 경로가 Cargo 출력
  경로와 같으면 오류다. 직접 소스 빌드 검증은 명시적 명령으로 유지하고 일상 검사는 검증된 파일을
  재사용하며 GN이나 Ninja를 실행하지 않는다. 의존: S-13-2-2-1. 증거: 누락·변조·동일 경로·검증된
  복사 사례, V8 소스 재빌드 없는 일상 검사 2회, 기존 벤치마크 한도 통과.
- [o] S-13-2-2-1-2 개발 이미지에서 공식 대상별 Linux V8 archive와 일치하는 생성
  binding을 검증한다. 의존: S-13-2-2-1-3. 필요한 증거: archive·binding digest 검증, 해당
  이미지에서 같은 스냅샷의 네이티브 병렬 렌더, V8 소스 컴파일 없는 두 번째 이미지 빌드.
  archive 다운로드나 소스 대체를 사용하지 않는다.
- [o] S-13-2-2-1-3 검증한 공식 로컬 V8 archive와 일치하는 binding을 사용하고 렌더 프로세스마다
  불변 스냅샷 키 하나를 유지한다. 우선순위: 렌더 서버 통합 전에 필요하다. 공개 스냅샷 생성기를
  완전히 정리한 뒤 같은 blob을 동시에 복원하고 초기화 한 번과 독립 요청 상태를 유지한다.
  모든 풀 종료 후에도 다른 키를 거부한다. 빌드와 렌더는 별도 Rust 프로세스에서 실행한다.
  의존: S-13-2-2-1. 필요한 증거: 추적되는 계약 RED, 공식 archive 네이티브 실행,
  기존 격리·성능 기준 유지, make check.
  추적되는 하위 프로세스 사례는 스냅샷 워커 실행 중 Svelte 컴파일러 진입을 호환되지 않는 V8
  isolate 생성 전에 거부해야 한다. 안전하게 완료한 빌드의 렌더 사용 경로는 유지한다.
  공통 프로세스 계약은 컴파일러와 스냅샷 초기화의 동시 실행을 막고 초기화 실패 뒤 생성기의 완전한
  정리를 검증한다. 테스트 소유 검사는 실제 공개 동작 사용마다 소비 사례를 대응하고 동작 또는 소비자
  선언 누락을 거부한다.
  Svelte 컴파일은 transform hook을 사용해 컴파일러와 프로세스 오류 원인을 번들러를 통해 보존하며
  공개 빌드 테스트에서 구체적인 원인을 요구한다.
  Cargo 사례 실행기는 컴파일 출력을 도착 즉시 전달하고 빌드 전체 기한을 두지 않는다. nextest의
  사례별 기한은 유지하며 누락되거나 실패한 사례 출력은 계속 오류로 처리한다.
  이 항목은 S-10, S-12, S-13-2-2-1, S-13-2-2-1-1의 같은 프로세스 다중 스냅샷과 소스 group
  요구를 교체하며 해당 항목에 기록한 완료 증거는 변경하지 않는다.
  증거: 공식 macOS AArch64 archive·binding을 사용한 make check가 종료 코드 0으로 통과했다.
  Python 54건, 소유·소비 사례 17건, 워크스페이스 145건, 기능 사례 12건, 벤치마크 단위 사례
  3건이 통과했다. make verify-build의 독립 빌드 사례 2건과 make bench의 기존 한도도 통과했다.
  엔진이 없는 빌드 검증 명령은 V8 입력을 요구하지 않으며 추적되는 회귀 사례가 이를 확인한다.
- [o] S-13-2-2-2 정확한 서버 진입점과 청크를 `ServerBundle`로 전달하여 V8 모듈로 평가한다.
  서버 진입점은 `render` 함수를 export하며
  전역 render 함수를 만들지 않는다. 생성한 서버 진입점과 풀의 모든 직접 호출자가 이 계약을 사용한다.
  정적·동적 상대 import는 정확한 비공개 서버 파일만 대상으로 해석하고,
  유효한 정적 import 순환을 유지하며, 누락 파일, 잘못된 경로, import 속성, 평가 실패와 끝나지 않은 최상위
  await를 거부한다. JavaScript 스택에는 각 서버 파일의 소스맵을 사용한다. 의존: S-5, S-6, S-9,
  S-10, S-13-2-2-1, S-13-2-2-1-1. 증거: 추적되는 다중 청크·동적 import·정적 순환·실패·전역 격리·스택 테스트와
  `make check` 통과.
- [o] S-14 notify 8.2.0으로 개발 모드를 구현한다: 파일이 바뀌면 번들을 다시 만들고 풀을 교체한다. 의존: S-9.
  완료 기준: 생성할 때 초기 서버를 빌드하고 절대 경로의 애플리케이션 루트를 감시한다. 소스 변경 시 모든 번들을
  다시 만들고 서버와 풀을 함께 교체한다. 재빌드 실패는 원인을 보고하고 이전 빌드를 제공하지 않는다. 이후 올바른
  변경이 생기면 서비스를 복구한다. 증거: 파일 이벤트 이후 렌더·공개 파일 출력 변경, 실패·복구 사례 통과.
- [o] S-15 Vue, Svelte, vanilla 어댑터와 `features.json`을 구현한다. 의존: S-15-1, S-15-2,
  S-15-3, S-15-4, S-15-5, S-15-6. 증거: 선언한 기능마다 실행 사례가 있다.
- [o] S-15-1 vanilla 어댑터를 구현한다. 의존: S-7. 완료 기준: 절대 서버·클라이언트 애플리케이션
  경로에서 번들 진입점을 만들고, SSR은 HTML과 출력 상태를 반환하며, CSR은 풀을 사용하지 않고 입력 상태를
  유지한다. 두 모드는 클라이언트 URL 하나를 사용하고, 정적 CSR 셸은 빈 props와 null 상태만 허용한다.
  잘못된 경로, URL, HTML, 입력은 오류다. 브라우저 시험은 hydration 후 서버 DOM 노드의 유지와 CSR
  렌더를 확인한다. 증거: 빌드·렌더·브라우저 사례와 `make check`.
- [o] S-15-2 Vue 어댑터를 구현한다. 의존: S-7, S-13-1. 완료 기준:
  애플리케이션 컴포넌트 하나에서 서버·클라이언트 진입점을 만들고, SSR은 Vue `renderToString`을 기다려
  HTML과 출력 상태를 반환하며, 브라우저는 기존 DOM 노드를 hydrate한다. CSR과 정적 셸은 S-15-1을
  따른다. 증거: 빌드·렌더·브라우저 사례와 `make check`.
- [o] S-15-3 Rust 빌드 프로세스 안에서 Svelte 애플리케이션 소스를 서버·클라이언트용으로 각각 컴파일하고
  Svelte 어댑터를 구현한다. 의존: S-7, S-15-3-1, S-13-2-2-2. 완료 기준: 소스 컴파일에 Node 프로세스가 필요 없고, 서버 진입점은 `render`를 ESM 함수로 export하며, SSR과 CSR은
  같은 클라이언트 빌드를 사용하며, 브라우저 hydration 후 서버 DOM 노드가 유지된다. 출력 상태와 정적 셸은
  S-15-1을 따른다. 컴포넌트 스타일은 내용 해시 CSS로 게시해 빌드 매니페스트에 포함하고 Svelte head
  출력은 문서 head에 포함한다. HTTP 서버는 Svelte 빌드를 ESM 풀로 제공하고 head와 본문의 인라인 script·style에
  요청 nonce를 적용하며 게시한 컴포넌트 CSS를 제공한다. 컴파일·빌드·렌더 오류는 명시적으로 실패한다. 생성한 JavaScript,
  CSS와 JavaScript 소스맵을 사용하며 컴파일러 경고는 실패한다. CSS 변환 뒤 CSS 소스맵은 게시하지 않는다.
  증거: 소스 빌드·HTTP 스타일·head·nonce 출력·렌더·브라우저 사례와 `make check`.
- [o] S-15-3-1 모든 런타임 렌더 결과에서 head 문자열을 필수로 받고 그 바이트를 `RenderResult`에
  전달한다. 의존: S-2, S-6. 완료 기준: 누락·문자열 아닌 값·잘못된 유니코드 head는 실패한다. React,
  Vue, vanilla 진입점은 빈 head를 반환하고 해당 어댑터는 문서 배치 기준이 없는 비어 있지 않은 head를
  거부한다. 증거: 런타임 결과·어댑터 거부 사례와 `make check`.

- [o] S-15-4 지원 기능을 `features.json`에 기록한다. 의존: S-7, S-15-1, S-15-2, S-15-3.
  완료 기준: 참으로 표시한 기능마다 정확한 테스트 패키지·바이너리·사례를 지정한다. 중복 선언,
  누락 사례, 명령 실패와 시간 초과는 실패하며 소스 검사만으로 기능을 추정하지 않는다.
  증거: 검사기가 참조한 각 사례를 정확히 실행하고 음성 검사 사례와 `make check`를 통과한다.
- [o] S-15-5 React 어댑터에서 빈 중첩 반복 폼을 검증한다. 우선순위: S-15 완료에 필수.
  의존: S-7, S-6, S-13.
  완료 기준: 추적되는 fixture `FormRowsApp.tsx`는 반복 자식 그룹을 담은 반복 행 그룹을 렌더한다. 한 서버 렌더의
  부모 행과 자식 행의 `rows.<key>.<field>` 이름은 같은 부모 키와 `row-`와 16진수 여덟 자리인 서로 다른 행 키를 사용하고, 같은 풀의 두 번째 렌더는 새 부모
  키를 만든다. 증거: 실행 가능한 두 렌더 사례와 `make check`.
- [o] S-15-6 React, Vue, Svelte, vanilla 클라이언트 진입점에서 명시적인 페이지 렌더 모드로
  hydration을 선택한다. 의존: S-7, S-15-2, S-15-3, S-13-2-2-2. 완료 기준: 서버 본문이 비어 있어도
  문서가 SSR 또는 CSR 모드를 보존하고, 클라이언트가 빈 SSR 본문에는 hydration을,
  CSR에는 새 렌더를 호출하며, 모드 누락과 잘못된 모드는 실패한다. 증거: 네 어댑터의
  실행 가능한 Chrome 사례와 `make check`.
- [o] S-15-6-1 React 진입점 빌드 테스트와 브라우저 hydration 테스트의 생성 파일을 격리한다.
  우선순위: 워크스페이스 검사의 안정성에 필요. 의존: S-15-6. 완료 기준: 동시에 준비한 테스트 진입점이
  다른 테스트의 서버·프레임워크·클라이언트 소스를 덮어쓰지 않으며, 두 테스트가 단독·병렬 실행에서
  각각의 출력을 빌드하고 검증한다. 증거: 추적되는 동시 덮어쓰기 사례가 수정 전 실패하고 수정 후
  통과하며, 영향받는 각 테스트와 병렬 실행, `make check`가 통과한다.
- [o] S-15-7 설정한 application dependency 디렉터리에서 모든 application package import를 해석해
  중첩된 `node_modules` 복사본이 모듈 상태를 중복 생성하지 않게 한다. 의존: S-7, S-15. 완료 기준:
  application source 디렉터리와 설정한 dependency 디렉터리에 같은 package가 있는 fixture가 server
  bundle에서 하나의 package 경로를 사용하고 render context provider와 consumer가 같은 module
  instance를 공유한다. 증거: 추적하는 resolver fixture가 수정 전 실패하고 수정 후 통과하며 소비하는
  SSR 요청이 missing-context 오류 없이 document를 반환한다.

- [o] S-15-7-1 표준 JavaScript resolver로 설정된 의존성 exports를 해결한다. 우선순위: 선언된 의존성 디렉터리를 소비하는 데 필요하다. 원인: 자체 의존성 resolver가 패키지 exports를 무시하여 export된 하위 경로를 거부한다. 수락 기준: 추적 RED가 범위가 지정된 조건부 export 실패를 재현하고 export되지 않은 하위 경로의 거부를 요구한다. 표준 resolver가 두 사례와 기존 단일 디렉터리 사례를 통과하며 의존성 디렉터리의 페이지·브라우저 사례가 통과한다. 의존: S-15-7.

- [o] S-16 각 로컬 체크아웃 경로를 루트 매니페스트에 한 번만 선언한다. 우선순위: 체크아웃을 옮긴 뒤 모든 빌드보다 먼저 필요하다. 원인: V8, CSS, JSON 체크아웃 경로가 세 매니페스트와 두 도구, engine Compose 템플릿에 반복되어 체크아웃을 옮기면 오래된 사본이 남고 다음 빌드에서야 실패한다. 수락 기준: 루트, build-probe, engine 매니페스트의 모든 절대 경로 의존성이 존재하는 체크아웃을 가리키고 같은 크레이트의 루트 매니페스트 경로와 일치한다. engine Compose 준비는 마운트 경로를 루트 매니페스트에서 읽는다. engine Compose 템플릿과 마운트 검사에는 호스트 경로가 없고 마운트 검사는 마운트 경로를 환경에서 읽는다. 증거: 추적 사례가 수정 전 존재하지 않는 V8, CSS, JSON 체크아웃 경로와 engine Compose 템플릿의 호스트 경로에서 실패하고 수정 후 로컬 경로 사례 여섯 개가 모두 통과한다. 잠긴 오프라인 Cargo metadata가 해결되고 `make check`가 통과한다. 의존: S-4, S-5, S-12.
- [o] S-17 끝난 React stream render의 scheduler를 해제한다. 우선순위: renderer 하나가 수백 개가 넘는 문서를 제공하기 전에 필요하다. 원인: 각 stream render는 slot에 scheduler를 두는 context를 만들고, scheduler는 render가 queue에 남긴 callback의 handle을 가지므로, 끝난 모든 render의 context가 도달 가능한 채로 남아 isolate가 수백 번의 render 뒤 heap을 다 쓴다. 수용 기준: 각 stream render 뒤에 성공과 실패 모두에서 scheduler는 queue의 callback을 버리고 context는 scheduler를 버린다. isolate 하나에서 stream render 600번이 heap을 초기 크기의 16 MiB 안에 유지한다. tracked heap test case가 변경 전 실패하고 변경 후 통과한다. 의존: 없음. 증거: heap test case는 변경 전 600번의 render 안에 V8 heap을 다 썼고 변경 후 통과한다.
- [o] S-18 테스트 실행이 테스트 규칙을 따르게 한다: 개발 중에는 변경을 소유한 Red·Green 테스트만 실행하고, 전체 묶음은 활성 체크리스트 항목이 모두 끝났을 때 한 번 실행하며, 모든 테스트 사례는 호스트 속도가 아니라 event와 자기 timeout으로 판정한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 의존: 없음. 수용 기준: S-18-1~S-18-6과 그 하위 항목이 완료된다. 전체 묶음은 `AGENTS.md`의 절차를 따른다. 활성 체크리스트 항목이 모두 끝난 뒤 `make check`를 한 번 실행하고, `var/full-run.json`이 그 결과와 단계별 시간을 기록하며, 실패는 새 체크리스트 항목이 된다. 수용 기준은 S-18이 진행 중인 동안 `make check` 통과를 요구했는데 `tools/full_run.py`(S-18-6)가 그 실행을 거부하므로, 전체 묶음 실행은 이 항목의 증거가 아니다. 증거: S-18-1, S-18-2, S-18-2-1, S-18-3, S-18-3-1, S-18-4, S-18-4-1, S-18-5, S-18-5-1, S-18-6이 완료되었다.
- [o] S-18-1 소유 테스트와 전체 묶음을 언제 실행하는지 적는다. 우선순위: 다음 커밋 전에 필요하다. 원인: 절차가 커밋마다 `make check`를, 항목을 완료로 표시할 때마다 전체 묶음을 요구하므로 수정마다 전체 묶음이 실행된다. 수용 기준: `AGENTS.md`와 `AGENTS.ko.md`가 커밋 전에는 변경을 소유한 Red·Green 테스트와 `tools/check.py`를, 전체 묶음은 활성 체크리스트 항목이 모두 끝났을 때 한 번 요구한다. Red와 Green: 이 문장을 읽는 검사가 없으므로 문서 변경으로 기록한다. 의존: 없음.
- [o] S-18-2 queue에 들어간 호출의 cancellation과 timeout 사례를 event로 판정한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: 두 사례는 pool이 대기 호출 하나를 보고할 때까지 `yield_now`로 반복하고 200 ms나 1 s가 지나면 실패하며, timeout 사례는 2.5 s가 지나도 실패하므로 느린 호스트에서 올바른 코드가 실패한다. 수용 기준: 테스트 전용 queue 진입 신호가 crossbeam channel로 호출의 queue 진입을 알린다. 두 사례는 그 신호를 기다리고 결과를 오류 종류로 판정하며, nextest의 30 s 한도 외에 시간 한도를 두지 않는다. cancellation 사례는 queue 대기를 cancellation만 끝낼 수 있도록 request timeout을 10 s로 둔다. Red: cancellation 호출이 queue에 들어가기 전 300 ms, timeout 호출이 들어가기 전 1100 ms의 테스트 전용 지연을 두면 두 사례가 `call did not enter queue`로 실패한다. Green: 같은 지연에서 두 사례가 통과한다. 의존: 없음.
- [o] S-18-2-1 deadline 하나가 queue 대기와 실행을 함께 덮는지 증명한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: S-18-2가 timeout 사례의 2.5 s 상한을 지워, 사례가 request deadline 하나와 queue 대기와 실행의 별도 deadline을 더 이상 구별하지 못한다. 수용 기준: 사례는 queue 대기 D 2 s, 실행 E 2 s, request timeout T 3 s를 써서 D < T, E < T, D + E > T가 된다. 같은 실행은 비어 있는 워커에서 완료되고, queue에 들어간 호출은 `Timeout`으로 끝난다. 경과 시간 상한 없이 결과만으로 판정한다. Red: deadline을 queue 대기만큼 늘려 실행에 별도 예산을 주면 queue에 들어간 호출이 성공하고 사례는 `matches!(waiting.join().unwrap(), Err(crate::Error::Timeout))`에서 실패한다. Green: request deadline 하나로 같은 사례가 5 s에 통과한다. 의존: S-18-2.
- [o] S-18-3 render benchmark 한도를 CPU time으로 판정한다. 우선순위: 다음 benchmark 실행 전에 필요하다. 원인: benchmark가 p99 latency, 초당 render 수, context 초기화 p99를 wall-clock 시간으로 판정하므로 호스트의 다른 작업이 올바른 runtime을 실패시킨다. 수용 기준: render는 pool 워커 thread에서 실행되어 호출 thread의 CPU time에 포함되지 않으므로, 각 호출은 고정한 `libc` crate의 `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)`로 process CPU time을 측정한다. CPU 초당 render 수, CPU p99, 최대 heap 변화를 판정하고 wall-clock 처리량, wall-clock p50과 p99, context 초기화 시간은 보고만 한다. `docs/benchmarks.md`가 정의, 측정 결과, 한도를 기록한다. `cargo deny check`가 통과한다. Red: wall-clock 표본은 한도보다 열 배 느리고 CPU 표본은 한도 안에 있는 `check_limits` 사례가 `concurrency=1 exceeded recorded benchmark limits`로 실패한다. Green: 같은 표본이 통과하고, 처리량, p99, heap 한도를 넘는 CPU 표본은 여전히 실패한다. load average가 약 150인 호스트에서 동시 호출 1의 wall-clock p99가 5.2 ms로 이전 2 ms 한도를 넘는 상태에서 `make bench`가 통과한다. 의존: 없음.
- [o] S-18-3-1 render CPU time을 워커 thread에서 측정한다. 우선순위: 다음 benchmark 실행 전에 필요하다. 원인: 동시 호출 4와 16에서 호출의 process CPU time에는 같은 시간에 실행되는 다른 호출의 CPU time이 포함되므로 그 시나리오의 CPU 한도가 render 하나를 측정하지 않는다. 수용 기준: `ssr-runtime`의 `bench` feature에서 워커는 고정한 `rustix` crate로 컨텍스트 초기화 시작부터 직렬화한 결과까지 `CLOCK_THREAD_CPUTIME_ID`를 측정해 `RenderMetrics::render_cpu`로 반환한다. 이 feature는 benchmark만 쓰고, `make check`는 benchmark를 이 feature로 빌드하고 lint하며, library는 `forbid(unsafe_code)`를 유지한다. benchmark는 CPU 처리량과 CPU p99를 이 워커 측정값으로 판정하며 한도는 모든 동시 호출에서 초당 1,000 render와 4 ms다. `cargo deny check`가 통과한다. Red: 동시 호출 16의 CPU p50이 동시 호출 1의 CPU p50의 네 배 미만이어야 하는 `cpu_time_of_a_call_excludes_concurrent_calls` 사례가 process CPU time에서 `CPU p50 alone=341µs concurrent=3.657ms`로 실패한다. Green: 워커 측정에서 같은 사례가 통과하고(`alone=512.834µs concurrent=316.75µs`), 호스트 load average 약 300에서 `make bench`가 CPU p99 0.70~1.04 ms로 통과한다. 의존: S-18-3.
- [o] S-18-4 browser 사례를 event로 판정하고 시간 한도는 멈춤 감지에만 쓴다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: browser 사례는 테스트 안에서 browser process를 25 s 한도로 기다리고 전역 nextest 한도 30 s 아래에서 실행되는데, 부하가 없는 호스트에서도 13~27 s가 걸린다. 호스트에 부하가 있으면 browser 종료만 11~30 s가 걸려 browser 검사가 모두 통과해도 사례가 실패한다. 수용 기준: 각 adapter test target은 `browser` test module로 browser script를 실행한다. 이 module은 script의 각 줄을 도착할 때 경과 시간과 함께 출력하고 browser process가 끝나면 종료한다. 테스트 안의 HTTP handler는 3 s read timeout 대신 browser가 연결을 닫을 때 끝난다. script는 page load, DOM 표시, page 오류를 기다리고, 시작, 검사하는 각 경로, browser 종료에 대해 `RUN`과 `DONE`을 출력하며, 단계 한도를 `browser-steps.mjs`에서 가져온다(단계 60 s, 시작 300 s로 평소 시간의 약 백 배). `test(/^browser_/)`에 대한 nextest override는 실행 중인 사례를 60 s마다 보고하고 30분 뒤 멈춘다. Red: 호스트 load average 약 300에서 React recovery, React, Vue, vanilla browser 사례가 모든 browser 검사가 `PASS`를 출력한 뒤 `browser timed out`으로 실패한다. Green: 호스트 load average 약 260~340에서 같은 네 사례가 24~48 s에 통과하고, 단계 출력은 browser 종료에 11~30 s가 걸림을 보인다. 의존: 없음.
- [o] S-18-4-1 browser 시작과 종료에 시간 한도를 두지 않는다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: S-18-4가 browser 시작에 300 s 한도(`browser-steps.mjs`의 `LAUNCH_TIMEOUT`)를 주었지만, browser 시작은 시간 한도 대신 단계 로그를 받는 긴 작업이다. 한도보다 느린 시작은 올바른 browser 사례를 결과가 아니라 시계로 실패시킨다. browser 종료에는 한도가 없다. 수용 기준: 다섯 browser script는 `timeout: 0`으로 browser를 시작하고 `RUN launch`와 `DONE launch`를 경과 시간과 함께 출력한다. `browser-steps.mjs`는 page 단계용 `STEP_TIMEOUT`(60 s)만 유지한다. `tools/test_browser_launch.py`는 3 s 늦게 시작하는 browser로 각 script를 실행하고, `node:test` module mock으로 1 s `LAUNCH_TIMEOUT`을 주입하며, 3 s 이상 뒤의 `DONE launch`, 첫 page 단계, `RUN close`와 `DONE close`를 요구한다. Red: 1 s 한도에서 다섯 script가 `browserType.launch: Timeout 1000ms exceeded`로 실패하고 `DONE launch`를 출력하지 않는다. Green: 다섯 사례가 37 s에 통과하고, 각 script는 약 3.8 s 뒤 시작을 보고하고 첫 page 단계를 시작하며 종료를 보고한다. 의존: S-18-4.
- [o] S-18-5 소유 사례를 한 번만 빌드한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: `tools/test_ownership.py`가 각 사례를 `cargo nextest run -p <crate>`로 실행해 그 패키지만으로 feature를 계산하므로, 공유 크레이트가 feature 조합마다 Cargo job 하나로 다시 컴파일되고 뒤의 워크스페이스 테스트 묶음이 한 번 더 컴파일한다. `make check`에서 이 단계는 597 s가 걸렸고 그중 400 s가 한 사례였다. 수용 기준: 검사기는 `cargo nextest run --workspace --no-run`으로 워크스페이스 테스트를 한 번 빌드하고, 빌드를 경과 시간과 함께 별도 단계로 보고하며 Cargo 진행을 전체 시간 제한 없이 전달한다. 그 뒤 각 사례를 지정한 filter로 `cargo nextest run --workspace` 한 번에 선언된 사례를 모두 실행한다. 각 사례는 자기 nextest timeout을 유지하고 통과나 실패로 보고된다. Red: 검사기가 `--workspace` 없이 `cargo nextest run -p ssr-core --test page`를 호출해 `test_cases_run_once_on_one_workspace_build` 사례가 실패한다. Green: 같은 사례가 통과한다. `ssr-core`를 바꾼 뒤 `CARGO_BUILD_JOBS=1`, 호스트 load average 150~360에서 측정했다. 이전에는 단계가 434 s 걸렸고 크레이트 컴파일이 23번이었으며 가장 느린 사례가 121 s와 117 s였다. 이후에는 268 s가 걸렸고 그중 빌드가 243 s, 크레이트 컴파일 8번이었으며 17개 사례 전체가 22.9 s, 가장 느린 사례가 20.6 s였다. 의존: 없음.
- [o] S-18-5-1 워크스페이스 metadata 읽기에 시간 제한을 두지 않는다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: `tools/test_ownership.py`가 `cargo metadata`를 30 s 제한으로 실행한다. Cargo는 여기서 index를 해석할 수 있으므로 이는 시간 제한 대신 단계 로그를 받는 긴 작업이며, 느린 읽기는 시계 때문에 검사를 실패시킨다. 수용 기준: 검사기는 `RUN cargo metadata ...`와 `PASS cargo metadata <s>s` 또는 `FAIL cargo metadata exit <code> <s>s`를 출력하고, Cargo 표준 오류를 도착하는 대로 전달하며, 시간 제한을 두지 않고, 0이 아닌 종료에서 `workspace metadata failed with exit <code>`를 발생시킨다. Red: Cargo를 실행하기 전 2 s 동안 출력이 없는 `cargo`와 1 s로 줄인 제한에서 `test_cargo_metadata_runs_past_any_limit_to_its_exit`가 `TimeoutExpired`로 실패하고, 101로 종료하는 `cargo`에서 `test_cargo_metadata_failure_names_its_exit_code`가 오류에 종료 코드가 없어서 실패한다. Green: 두 사례가 통과하고, 첫 사례는 지연 뒤 `PASS cargo metadata 2.x s`를 보고한다. `tools/test_test_ownership.py`의 27개 사례가 5.4 s에 통과한다. 의존: S-18-5.
- [o] S-18-6 실행할 수 없는 전체 묶음을 어떤 단계보다 먼저 거부한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: `make check`는 checkout의 상태와 상관없이, 진행 중인 항목이 있어도, commit하지 않은 변경이 있어도, 전체 묶음이 이미 실행된 tree에서도 단계를 시작했으므로 S-18-1의 규칙은 실행하는 사람이 지킬 때만 지켜졌다. target 없는 `make`는 checkout lock 없이 `check-steps`를 바로 실행했다. 수용 기준: `tools/full_run.py`가 `make check`와 `make rerun-failed`의 어떤 단계보다 먼저 결정하고 그 결정을 이유와 함께 출력한다. `docs/checklist.md`의 항목(하위 항목 포함)이 진행 중인 동안(각 ID와 제목을 적는다), 추적하는 file에 commit하지 않은 변경이 있는 동안, 그리고 `make check`이면 `var/full-run.json`이 같은 tree(`git rev-parse HEAD^{tree}`)의 전체 실행을 기록하고 있을 때(그 실행을 적는다) 종료 상태 2로 거부한다. 기록은 tree, commit, 결과, 통과하지 못한 target과 모든 단계의 시각을 담고, 첫 단계 전과 각 단계의 시작과 끝마다 쓰이므로 강제 종료된 실행은 `incomplete`로 남는다. `make rerun-failed`는 setup 단계와 현재 tree의 기록에서 통과하지 못한 target만 실행하며, 그런 기록이 없으면 거부된다. 이전 `check-steps`의 단계는 `CHECK_SETUP`과 `CHECK_TARGETS`의 make target이다. target 없는 `make`는 `make check`를 실행한다. Red: `tools/full_run.py`가 있고 Makefile이 이전 것일 때 `test_make_check_starts_with_the_guard`가 실패했다. `check` recipe가 `$(MAKE) check-steps`를 바로 실행하므로 진행 중인 항목이 있는 checklist에서도 단계가 시작된다. target 없는 `make`의 기본 목표는 `check-steps`였다. Green: `tools/test_full_run.py`의 11개 사례가 stub make target을 둔 임시 checkout에서 통과한다: 활성 항목과 활성 하위 항목은 거부되어 나열되고 단계는 실행되지 않으며, commit하지 않은 Makefile 변경은 거부되고, 같은 tree의 두 번째 전체 실행은 첫 실행을 적으며 거부되고, 기록 없는 rerun은 거부되고, rerun은 setup 단계와 실패한 target `b`만 실행한 뒤 다시 실행할 것이 없으며, SIGKILL로 종료된 실행은 target이 `running`인 `incomplete`로 남는다. full run, check lock, run directory, archive, feature, workspace 도구 테스트 38개 사례가 통과한다. 이 checkout의 `make check`는 S-9-1-1과 S-18, commit하지 않은 file을 나열하며 종료 상태 2로 거부되고 어떤 단계도 실행하지 않는다. 의존: S-18-1.
- [o] S-19 검증 실행 하나의 resource를 다른 실행으로부터 지킨다. 우선순위: 다음 engine 검증 전에 필요하다. 원인: 다른 checkout이나 같은 checkout의 동시 실행이 같은 container stack과 같은 fixture 설치를 쓴다. 수용 기준: S-19-1과 S-19-2가 완료된다. 의존: 없음.
- [o] S-19-1 checkout마다 engine 검증 stack을 따로 두고 lock으로 잡는다. 우선순위: 다음 engine 검증 전에 필요하다. 원인: engine 검증은 고정된 Compose project 이름, 고정된 container 이름, image `localhost/ssr-engine-verify:0.0.1`을 써서, 다른 checkout의 검증이 실행 중인 stack을 바꿨다. 수용 기준: `tools/prepare_engine_compose.py`는 project를 `ssr-engine-<checkout path SHA-256의 16진수 12자리>`, container를 `<project>-engine`, image를 `localhost/ssr-engine-verify-<자리>:0.0.1`로 이름 짓는다. `make verify-engine-linux-arm64`와 `make verify-engine-down`은 `tools/verify_engine_linux.py`를 실행하며, 이 도구는 모든 단계 동안 `tools/holder_lock.py`의 checkout lock `var/locks/engine-verification.lock`을 잡고 각 단계를 결과와 경과 시간과 함께 시간 제한 없이 출력한다. 잡힌 lock은 holder의 checkout, pid, process 시작 시각과 함께 거부한다. holder가 더 이상 실행되지 않는 lock은 보고하고 `python3 -m tools.holder_lock remove-stopped`가 지울 때까지 남겨 둔다. holder만 lock을 해제한다. engine Compose 사례는 checkout의 `var/engine-compose.yaml`을 쓰지 않고 file을 만든다. Red: `tools/test_engine_stack.py`가 `tools.verify_engine_linux`를 import하지 못했고, Makefile, Compose template, status 검사가 고정된 project와 `ssr-engine-verify:0.0.1`을 썼으며, `tools/test_holder_lock.py`가 `tools.holder_lock`을 import하지 못했다. Green: stack 사례 4개와 lock 사례 9개가 통과하고, local path 사례 6개가 만든 file로 통과한다. `python3 -m tools.prepare_engine_compose`는 worktree에 project `ssr-engine-98e175b66c0f`를 쓴다. 의존: 없음.
- [o] S-19-2 `make check` 전체 동안 checkout의 fixture 설치를 잡는다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 원인: build-probe fixture 설치는 checkout마다 directory 하나이고, 같은 checkout에서 동시에 실행되는 두 번째 `make check`가 첫 실행의 test가 읽는 동안 그것을 다시 설치했다. 수용 기준: `make check`는 `python3 -m tools.holder_lock run check -- $(MAKE) check-steps`를 실행하며, 이 command는 첫 설치부터 마지막 test까지 `var/locks/check.lock`을 잡고, SIGINT, SIGTERM, SIGHUP을 단계에 전달하며, 성공과 실패 모두에서 단계가 끝난 뒤 lock을 해제하고, 두 번째 실행은 holder의 record와 함께 거부한다. target별 export와 archive 검사는 `check-steps`에 적용된다. `make check` 밖의 test 실행은 lock을 잡지 않으며, fixture는 `make check`만 준비한다. Red: `tools/test_check_lock.py`의 사례 3개가 실패했다: `check` recipe는 단계를 직접 실행했고 `holder_lock`에는 `run`이 없었다. Green: 사례 3개가 통과하고(command 실행 중 lock이 있고, exit 0과 exit 4 뒤 lock이 없으며, 잡힌 lock은 command를 실행하지 않고 거부한다), `tools/test_verify_archive.py`는 `check-steps bench: verify-archive`를 기대하며, Makefile을 읽는 도구 test 22개가 통과한다. 의존: S-19-1.
- [o] S-20 체크리스트에서 상태 표시는 항목의 상태로만 쓴다. 우선순위: 도구가 체크리스트 상태를 다시 읽기 전에 필요하다. 원인: 체크리스트가 네 상태 표시의 범례로 시작하고 S-18-1과 S-18-6의 문장이 inline code로 표시를 적었는데, `tools/check.py`는 영어와 한국어 체크리스트의 항목 상태만 비교했으므로 표시를 세는 도구가 존재하지 않는 진행 중 항목을 셌다. 수용 기준: `AGENTS.md`와 `AGENTS.ko.md`가 상태를 정의한다. `docs/checklist.md`와 `docs/checklist.ko.md`에는 범례가 없고 문장은 상태를 말로 적는다. `tools/check.py`는 두 file에서 항목 줄의 상태가 아닌 모든 대괄호 상태 표시에 대해 실패하며, 항목 줄은 하이픈, 공백, 표시, 공백, 항목 ID로 시작하는 줄이다. 실패는 file, 줄, 열과 이유를 적고 예외나 허용 목록을 두지 않는다. Red: `tools/test_checklist_markers.py`의 fixture는 범례, table 칸, 항목의 inline code, 이어지는 줄의 문장, 항목 줄 안에 적힌 두 번째 항목에 표시를 담는다. `tools/check.py`의 이전 `check_pairs_and_links`는 이 fixture에 오류를 내지 않았으므로 `test_marker_outside_an_item_state_names_its_location`이 기대한 오류 12개 대신 빈 목록을 받아 실패했다. Green: 3개 사례가 통과한다. fixture는 각 file의 위치 6개로 실패하고, 표시를 항목 상태로만 쓰는 체크리스트는 통과하며, 저장소 체크리스트는 통과한다. 검사를 넣고 체크리스트가 이전 것일 때 `python3 tools/check.py`는 표시 14개(영어 file의 5, 362, 371번 줄과 한국어 file의 5, 304, 313번 줄)를 적으며 종료 상태 1로 끝났고, 범례를 없애고 문장을 고친 뒤에는 통과한다. 의존: 없음.
- [o] S-20-1 GitHub의 task list 상태도 상태 표시로 다룬다. 우선순위: 도구가 체크리스트 상태를 다시 읽기 전에 필요하다. 원인: S-20은 이 저장소의 네 상태만 검사하지만, Markdown reader는 대괄호 안의 x나 대문자 X도 task list 항목의 상태로 읽으므로, 항목 상태 밖의 그런 표시가 `tools/check.py`를 통과했다. 수용 기준: `tools/check.py`는 `docs/checklist.md`와 `docs/checklist.ko.md`에서 항목 줄의 상태가 아닌 대괄호 안의 공백, 물결표, o, 느낌표, x, 대문자 X에 대해 file, 줄, 열을 적고 실패한다. 항목 줄은 `AGENTS.md`가 정의하는 네 상태를 유지하므로, x 형식으로 시작하는 줄은 항목 줄이 아니다. Red: `tools/test_checklist_markers.py`의 fixture에 x 형식의 항목 줄과 inline code의 대문자 X를 더하자 이전 검사가 10, 11번 줄의 기대 오류 4개를 하나도 내지 않아 `test_marker_outside_an_item_state_names_its_location`이 실패했다. Green: 3개 사례가 통과하고 저장소 체크리스트에서 `python3 tools/check.py`가 통과한다. 의존: S-20.
- [o] S-20-2 체크리스트에는 항목과 제목만 둔다. 우선순위: 도구가 체크리스트를 다시 읽기 전에 필요하다. 원인: 체크리스트가 번역 문서 link, 절차를 적은 문단, 요구 사항 section으로 시작했고 그런 문장을 항목과 구별하는 검사가 없었으므로, file을 읽는 쪽이 모든 목록 항목을 항목으로 받아들일 수 없었다. 수용 기준: 요구 사항은 S-0-1이 link하는 `docs/requirements.md`와 `docs/requirements.ko.md`에 있다. 문단이 적던 절차는 `AGENTS.md`와 `AGENTS.ko.md`에 있다. `tools/check.py`는 `docs/checklist.md`와 `docs/checklist.ko.md`에서 빈 줄, 제목, 항목 줄, 항목의 이어지는 줄이 아닌 모든 줄에 대해 실패하고 file, 줄, 열을 적는다. 이어지는 줄은 항목 줄이나 다른 이어지는 줄 바로 다음에 오는, 공백 두 칸으로 들여 쓴 줄이다. Red: 번역 link, 문단, 제목 아래의 목록 항목과 그 두 번째 줄, 빈 줄 다음의 들여 쓴 문단을 담은 fixture에 이전 검사가 오류를 내지 않아 `test_text_outside_an_item_names_its_location`이 실패했다. Green: 4개 사례가 통과하고 fixture는 file마다 위치 5개로 실패한다. 검사를 넣고 체크리스트가 이전 것일 때 `python3 tools/check.py`는 영어 file의 1~21번 줄과 한국어 file의 1~17번 줄에 대해 실패했고, 요구 사항을 `docs/requirements.md`로 옮기고 문단과 link를 없앤 뒤에는 통과한다. 의존: S-20-1.
- [o] S-21 체크리스트 항목이 진행 중인 동안 push를 거부한다. 우선순위: 다음 push 전에 필요하다. 원인: `origin`이 `github.com/polyspec/ssr`인데도 `AGENTS.md`가 저장소를 로컬 전용으로 적었고, 진행 중 항목이 있는 tree의 push를 거부하는 것이 없었으므로, 진행 중 항목이 없을 때만 push한다는 규칙은 push하는 사람이 적용할 때만 지켜졌다. 수용 기준: 추적되는 pre-push hook `.githooks/pre-push`가 `python3 -m tools.push_gate hook`을 실행하고, 이것은 push되는 모든 commit과 작업 트리의 체크리스트를 `tools/full_run.py`의 `active_items`로 읽어 각 항목을 원격 ref, commit, ID, 제목과 함께 적고 종료 상태 1로 push를 거부하며, push되는 commit에 체크리스트가 없으면 거부한다. 모든 make 실행은 `core.hooksPath`가 `.githooks`와 다르면 그것으로 설정하고, `make hooks`가 설정하고 검사하며, 설정되지 않은 동안 `tools/check.py`와 `tools/full_run.py`의 결정이 실패한다. workflow `.github/workflows/push-gate.yml`은 모든 push와 pull request에서 job `push-gate`로 `python3 -m tools.push_gate commit HEAD`를 실행하고, 같은 항목을 error annotation과 job summary에 적고 실패하며, commit이 hook을 mode 100755로 추적하지 않을 때도 실패한다. Red: hook과 도구가 없을 때 `tools/test_push_gate.py`의 11개 case가 실패했고(`.githooks/pre-push` 없음) `tools/test_full_run.py`의 13개 중 11개 case가 실패했다(fixture의 hook 없음, hook 이유가 없는 `decide`). 작업 트리를 읽지 않게 하면 작업 트리 case가 실패했고, `check_hooks`가 오류를 돌려주지 않게 하면 기록 검사 case가 실패했다. Green: `tools/test_push_gate.py`의 12개 case가 임시 bare 원격으로 통과한다. 항목이 없는 push는 원격 ref를 갱신한다. S-2와 S-2-2가 진행 중인 commit의 push는 둘을 ref, commit과 함께 적고 거부되며 원격 ref는 바뀌지 않는다. 작업 트리에만 진행 중 항목이 있어도 깨끗한 commit의 push가 거부된다. 체크리스트가 없는 commit의 push는 거부된다. branch 삭제는 작업 트리에 항목이 있는 동안 거부되고 없으면 허용된다. `hooks-check`는 `make hooks`가 경로를 설정하기 전까지 실패하고, `make -n`이 경로를 설정하며, 실행 bit가 없는 hook은 실패한다. `commit HEAD`는 진행 중 항목에 대해 error annotation과 job summary를 적고 1로 끝나고, mode 100644 hook과 추적되지 않는 hook에 대해 1로, 깨끗한 commit에 대해 0으로 끝난다. `tools/test_full_run.py`의 13개 case가 통과하며 guard는 `core.hooksPath`가 없는 checkout을 어떤 단계보다 먼저 거부한다. 체크리스트 표시 case 4개와 Makefile을 읽는 도구 테스트 31개 case가 통과하고, `make hooks`와 `tools/check.py`가 통과한다. 의존: 없음.
- [o] S-21-1 push 검사가 push한 tip을 읽는다고 적는다. 우선순위: 다음 push 전에 필요하다. 의존: S-21. 원인: S-21의 기록은 hook과 workflow가 push한 모든 commit의 checklist를 읽는다고 쓰지만, 실제로는 push한 tip의 checklist를 읽으며 이것이 의도된 규칙이다. 항목은 한 commit에서 진행 중으로 기록되고 다음 commit에서 완료되므로 push의 앞선 commit에는 진행 중 항목이 있다. 수용 기준: `AGENTS.md`, `tools/push_gate.py`의 docstring, `.github/workflows/push-gate.yml`의 주석과 changelog가 merge commit이 아니라 push한 각 ref의 tip과 pull request의 head commit을 적는다. Red와 Green: 이 문장을 읽는 검사가 없으므로 문서 변경으로 기록한다. `tools/test_push_gate.py`(12개 사례)와 `tools/check.py`가 통과한다.
- [o] S-22 전체 묶음에서 `ssr-server`를 Linux에서 native로 빌드하고 lint하고 test한다. 우선순위: 다음 전체 묶음 실행 전에 필요하다. 의존: S-19-1. 원인: 라이브러리는 Linux용으로 배포되지만 `make check`는 macOS host target만 컴파일하므로, S-9-1-2-1이 저장소 밖의 compiler wrapper로 직접 컴파일하기 전까지 Linux 소스 감시는 Linux에서 컴파일되지 않았다. 그 wrapper는 Linux 사용자가 쓰지 않는 두 번째 toolchain이다. 수용 기준: `CHECK_TARGETS`의 target `check-linux`는 `tools/check_linux.py`를 실행한다. 이 도구는 `tools/holder_lock.py`의 checkout lock `engine-verification`을 잡고, Linux gcc toolchain, clippy를 포함한 Rust 1.98.1, cargo-nextest 0.9.146이 있는 `verification/engine/linux/Dockerfile`로 만든 checkout의 container stack(S-19-1)에서 실행한다. checkout과 선언된 경로 의존 checkout 세 개는 읽기 전용으로 mount하고 Cargo cache와 빌드 출력만 쓸 수 있다. 단계는 AArch64 Linux V8 입력 검증, Compose file 준비, image 빌드, stack 시작, 상태와 mount 확인, 잠긴 의존성 fetch, `cargo clippy --locked -p ssr-server --all-targets -- -D warnings`, `development_process`와 `socket_process` 예제 빌드, `cargo nextest run --locked -p ssr-server --lib --test development --test process --no-tests fail`, stack 중지다. 각 단계는 명령, 도착하는 출력, 경과 시간과 함께 결과를 출력하고 시간 제한이 없으며, 실패한 단계는 target을 실패시킨다. checkout의 build-probe fixture에 생성 entry를 쓰는 `ssr-server` test target은 읽기 전용 mount에 쓸 수 없어 이 target에 들어가지 않는다. 그 코드에는 platform별 경로가 없다. stack이 시작된 뒤에는 단계가 끝나면, 실패한 단계 뒤에도 stack을 멈춘다. Red: `tools/test_check_linux.py`가 `cannot import name 'check_linux'`로 실패했다. S-9-1-2의 source와 도구로 target은 archive, Compose, image(453 s), 시작, 상태, mount, fetch(94 s) 단계를 통과하고 `cargo clippy`에서 138 s 뒤 `unresolved import notify::Event`, `cannot find type Result in module notify`, `enum import RecursiveMode is private`로 실패했다. 그 실행은 stack을 남겨 직접 멈췄고, 그때 실패 뒤의 중지를 추가했다. Green: 실패한 단계 뒤에 중지가 따르는 사례를 포함한 `tools/test_check_linux.py` 사례 4개와 `tools/test_engine_stack.py` 사례 4개가 통과한다. main branch에서 `python3 -m tools.check_linux`가 통과한다. image 9.9 s, 시작 23.7 s, fetch 44.3 s, clippy 140.4 s, 예제 빌드 317.1 s, Linux에서 `ssr-server` 단위, `development`, `process` 사례 39개 중 39개가 통과한 nextest 194.9 s, 중지 4.5 s.
- [o] S-22-1 mount 검사 사례가 CSS checkout을 요구하게 한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-22. 원인: S-22는 `tools/verify_engine_mounts.py`가 요구하는 mount에 `SSR_LIGHTNINGCSS_DIR`를 더했지만 `tools/test_local_paths.py`의 `test_mount_check_requires_absolute_environment_paths`는 V8과 JSON 경로만 설정했으므로, 이 사례는 `main`에서 `SSR_LIGHTNINGCSS_DIR must name an absolute checkout path`로 실패했다. 수용 기준: 사례는 선언된 세 경로를 설정하고, 상대 경로인 각 경로에서 실패하며, 각각을 읽기 전용 mount로 요구한다. Red: 사례가 `main`에서 그 오류로 실패했다. Green: `tools/test_local_paths.py`의 6개 사례가 통과한다.
- [ ] S-23 같은 tree가 언제 어느 machine에서든 같은 결과를 낸다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: 없음. 원인: 검사 결과가 추적되지 않는 파일, 오래된 build 출력, registry 조회 시점, 고정되지 않은 도구, 공유 fixture, 도구 출력 형식에 따라 달라진다. 수용 기준: S-23-1부터 S-23-11까지 완료되고 `AGENTS.md`와 `AGENTS.ko.md`가 idempotency 규칙을 적는다.
- [o] S-23-1 추적되지 않는 파일이 있으면 full run을 거부한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: 없음. 원인: `tools/full_run.py`의 guard는 `git status --porcelain --untracked-files=no`를 읽으므로, add하지 않은 module처럼 build가 읽는 추적되지 않는 파일이 tree로 기록되는 full run에 참여하고, `tools/check.py`는 `rglob`으로 추적되지 않는 Markdown과 Rust 파일을 읽는다. 수용 기준: 추적되지도 무시되지도 않는 파일(`git ls-files --others --exclude-standard`)이 있으면 guard가 `make check`와 `make rerun-failed`를 거부하고 각 파일을 적는다. `tools/check.py`는 추적되는 파일만(`git ls-files`) 읽고, 무시되지 않는 추적되지 않는 파일마다 실패하며, checkout을 읽을 수 없으면 Git 오류를 적고 실패한다. `tools/test_ownership.py`는 guard가 지키는 full run의 target으로만 실행되므로 guard가 그 읽기를 지킨다. Red: fixture checkout에 추적되지 않는 `docs/module.rs`가 있을 때 `tools/test_full_run.py`의 `test_untracked_file_refuses`는 `1 != 2`로 실패했다. guard가 실행을 허용하고 target을 실행했다. 이전 `tools/check.py`에서 `test_untracked_and_ignored_documents_are_not_read`는 추적되지 않는 `notes.md`가 빠진 짝과 깨진 link를 더해 `4 != 2`로 실패했다. Green: 두 사례가 통과한다. `tools/test_full_run.py`의 14개, `tools/test_workspace.py`의 10개, checklist marker 4개, React 문서 2개, `tools/test_push_gate.py`의 12개 사례와 check lock, run directory 사례가 통과하고 `tools/check.py`가 통과한다.
- [o] S-23-2 test build가 컴파일한 예제 프로그램을 실행한다. 우선순위: `ssr-server`의 다음 owning test 실행 전에 필요하다. 의존: 없음. 원인: `ssr-server`의 development와 socket test는 `target/debug/examples/development_process`와 `socket_process`를 실행했는데, Cargo는 target을 선택하지 않을 때만 이것을 build하므로 `cargo nextest run -p ssr-server --test development`나 `--lib`는 이전 소스로 build된 프로그램을 실행하거나, 프로그램이 없으면 실패했다. 수용 기준: `ssr-server`의 어떤 test보다 먼저 `.config/nextest.toml`의 nextest setup script `build-programs`(고정된 nextest 0.9.146의 실험 기능 `setup-scripts`)가 `tools/build_programs.py`를 실행하고, 이것은 두 예제를 `cargo build --locked -p ssr-server`로 build하고 Cargo가 보고한 실행 파일을 `NEXTEST_ENV`에 `SSR_DEVELOPMENT_PROCESS`와 `SSR_SOCKET_PROCESS`로 쓴다. test는 이 변수에서만 프로그램을 실행하고 변수가 없으면 그 이름을 적고 실패한다. 각 프로그램은 `source-digest`에 자기 소스의 SHA-256을 출력하고, `programs_are_built_from_the_current_sources`는 이를 소스 파일과 비교한다. script filter가 `ssr-server`를 가리키므로 `tools/build-probe`는 자기 nextest 설정을 가지고, `make verify-build`는 저장소 설정을 더 넘기지 않는다. Red: 프로그램을 build한 뒤 `tests/support/socket_process.rs`에 한 줄을 더하자 `cargo nextest run -p ssr-server --test development`에서 `programs_are_built_from_the_current_sources`가 `socket_process was built from other sources than tests/support/socket_process.rs`로 실패했다. Green: 같은 변경에서 script가 프로그램을 5.0 s에 다시 build했고 사례가 통과했다. `cargo nextest run -p ssr-server --lib --test development --test process`의 47개 사례가 통과한다. `tools/test_build_programs.py`의 4개 사례와 archive, full run, Linux 검사 사례가 통과한다. `tools/build-probe`의 `cargo nextest list`가 자기 설정을 읽는다. `cargo fmt --all -- --check`와 `cargo clippy -p ssr-server --all-targets -- -D warnings`가 통과한다.
- [o] S-23-3 결과에서 시간, registry, 고정되지 않은 도구를 없앤다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-22. 원인: `cargo deny check`는 시간에 따라 바뀌는 보안 advisory database를 읽었다. `test_license_exceptions_are_exact_versions`는 `cargo generate-lockfile`를 실행했다. rustup은 `make -n`의 `rustc -vV`에서도 없는 toolchain을 필요할 때 설치했다. node, npm, Python, make, cargo-deny, zig, browser, container 도구는 고정되지 않았다(같은 machine의 한 directory에서 npm은 12.2.0, 다른 directory에서 11.19.0을 보고했다). 수용 기준: `make check`는 workspace, engine 검증, build 검증에 `cargo deny check bans licenses sources`를 실행하고, `CHECK_TARGETS` 밖의 `make review-advisories`가 그 advisories를 검사한다. license 사례는 `tools/license-fixtures` 아래의 추적되는 package와 추적되는 lock을 offline으로 읽는다. 모든 make 호출은 `RUSTUP_AUTO_INSTALL=0`을 export하고 그것과 함께 `rustc -vV`를 실행한다. `tools/tool-versions.json`은 python3, make, rustc, cargo-nextest, cargo-deny, node, npm, zig, browser, container, containerctl(SHA-256으로)을 선언하고, `tools/tool_versions.py`는 `make check`와 `make rerun-failed`의 guard, `tools/check.py`, `bench`, `verify-build`, `verify-engine-linux-arm64`, `review-advisories`의 첫 command에서 이를 검사하며 pre-push hook에서 Python을 검사하고, 각 실패는 기대 보고와 실제 보고를 적는다. GitHub의 workflow는 다른 Python으로 실행된다. S-23-3-1이 그것을 고정한다. Red: 이전 Makefile에서 `tools/test_tool_versions.py`의 Makefile 사례 3개가 실패했다(첫 command가 `python3 tools/bench.py`, `RUSTUP_AUTO_INSTALL` 없음, advisories를 포함한 `cargo deny check`). Green: `make check-deny`와 `make verify-engine-deps`가 advisory database를 fetch하지 않고 `bans ok, licenses ok, sources ok`를 출력한다. `tools/test_tool_versions.py`의 9개, `tools/test_workspace.py`의 11개, `tools/test_full_run.py`의 15개, `tools/test_push_gate.py`의 13개 사례와 check lock, archive, engine stack 사례가 통과하고, `python3 -m tools.tool_versions check`와 `tools/check.py`가 통과한다.
- [ ] S-23-3-1 GitHub의 push 검사를 선언된 Python으로 실행한다. 우선순위: 다음 push 전에 필요하다. 의존: S-23-3. 원인: workflow `push-gate`는 `ubuntu-26.04-arm` image의 `python3`을 실행하지만 `tools/tool-versions.json`은 macOS의 Python 3.9.6을 선언하고, `actions/setup-python`에는 arm64 Ubuntu 26.04용 3.9.6 build가 없으므로(그 manifest는 3.9.6을 x64의 Ubuntu 18.04와 20.04에만 둔다) 로컬 hook과 workflow가 다른 Python version으로 실행된다. 수용 기준: 이 host와 workflow에서 같은 방식으로 준비한 Python version 하나(예: 고정된 uv와 그것이 관리하는 Python)가 모든 Python 도구를 실행하고, 선언이 그것을 적으며, workflow는 `tools/push_gate.py commit` 전에 그것을 검사한다. Red: workflow job이 선언과 다른 Python을 보고한다. Green: job과 로컬 hook이 선언된 Python을 보고한다.
- [ ] S-23-4 실행마다 build-probe package와 생성 entry를 따로 둔다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-22. 원인: `make check`의 `npm ci`는 `tools/build-probe/tests/fixtures/node_modules`를 지우고 다시 쓰는데 owning test 실행은 lock 없이 이를 읽고, test는 그 아래 고정 directory에 생성 entry를 쓰므로 한 checkout의 두 실행이 서로 덮어쓴다. 수용 기준: package는 실행의 directory에 설치되어 rename 한 번으로 게시되고, test는 lock과 대조한 뒤에만 설치된 package를 읽으며, 각 test는 자기 directory에 생성 entry를 쓰고 그것을 지운다. Red: 설치 중에 package를 읽는 사례와 한 test의 두 실행이 서로의 entry를 덮어쓰는 사례. Green: 두 사례가 통과한다.
- [ ] S-23-5 바뀐 모든 파일을 owning test에 대응시킨다. 우선순위: 다음 commit 검사 전에 필요하다. 의존: 없음. 원인: `tools/test-ownership.json`은 `ssr-core`와 `ssr-build`에만 owner를 선언하므로 `ssr-runtime`, `ssr-nonce`, adapter의 public name에는 선언된 owner와 소비자 test가 없고, 바뀐 도구, Makefile, workflow, fixture의 owning test를 고르는 map이 없다. `tools/run_tests.py`와 `tools/verify_engine_status.py`에는 test가 없다. 수용 기준: 소비자가 있는 모든 crate가 선언된 owner이고, 추적되는 map이 추적되는 모든 파일의 owning test를 적으며 도구는 그것이 없는 파일에서 실패한다. 두 도구에 test가 있다. Red: owning test가 없는 추적 파일이 통과한다. Green: 그 파일이 실패하고 추적되는 모든 파일이 대응된다.
- [o] S-23-6 test와 검증 단계의 process와 directory를 회수한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-22. 원인: `tools/verify_engine_linux.py`는 첫 실패 단계에서 `containerctl down` 없이 돌아왔고, S-22의 `tools/check_linux.py`는 stack을 멈추는 단계 loop를 따로 복사해 두었다. `tools/run_tests.py`와 `tools/test_browser_launch.py`는 timeout에서 직접 child만 죽였으므로 node, browser, make가 계속 실행되었다. Rust test는 assertion이 통과한 뒤에만 임시 directory를 지웠다. `console_writes_to_stderr`는 읽지 않는 standard error pipe를 가진 child를 기다렸다. 수용 기준: `tools/verify_engine_linux.py`의 `run_locked`는 시작된 stack을 단계가 끝난 뒤, 실패 단계 뒤에도 멈추고, 실패한 단계와 멈춤의 결과를 보고하며 첫 실패를 돌려준다. `tools/check_linux.py`와 `make verify-engine-linux-arm64`는 이것과 `stop`을 쓴다. `tools/run_tests.py`의 `run_case`는 각 도구 test와 `tools/test_browser_launch.py`의 browser script를 자기 process group에서 실행하고, 결과, 종료 상태, 두 출력을 돌려주며, 끝과 제한에서 group을 죽인다. `crates/ssr-build/tests/build.rs`와 `public.rs`, `crates/ssr-adapter-react/tests/react.rs`, `supervisor_tests.rs`의 합치기 사례의 임시 directory는 실패에서도 지우는 guard이고 새로 만들어지므로(`create_dir`) 이전 실행이 남긴 directory는 사례를 실패시킨다. console 사례는 기다리는 동안 standard error를 읽는다. Red: `tools/test_engine_stack.py`의 `test_a_started_stack_is_stopped_after_a_failing_step`와 `test_verification_stops_the_stack_it_started`가 실패했다(`run_locked() takes from 1 to 2 positional arguments`, `stop` 없음). `tools/run_tests.py`의 이전 `subprocess.run(timeout=2)`는 2.0 s 뒤 돌아오며 손자 process를 남겼고, `tools/test_run_tests.py`는 `run_case`가 없어 실패했다. `Temporary`의 `Drop`을 지우면 `a_failed_assertion_removes_the_temporary_directory`가 `the directory of a failed case remains`로 실패했다. Green: `tools/test_engine_stack.py`의 6개 사례, `tools/test_check_linux.py`의 사례, `tools/test_run_tests.py`(2개), `tools/test_browser_launch.py`(5개)가 통과한다. `ssr-build::build`의 8개 사례, public file 사례, React 동시성과 의존성 사례, `console_writes_to_stderr`, `events_during_a_build_start_one_follow_up_build`를 포함한 owning Rust 사례 14개가 통과하고, `cargo fmt --all -- --check`와 `cargo clippy -p ssr-build -p ssr-adapter-react -p ssr-runtime -p ssr-server --all-targets -- -D warnings`가 통과한다.
- [ ] S-23-7 한 실행의 모든 검사를 보고한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-22. 원인: `make check`의 setup 단계가 실패하면 그 출력을 읽지 않는 target까지 모든 target을 건너뛰고, `verify-build`의 recipe는 첫 실패 명령에서 멈추며, `tools/test_ownership.py`는 첫 선언 오류에서 멈춘다. 수용 기준: 실패한 setup 단계는 그 출력을 읽는 target만 그 이유와 함께 건너뛰고, `verify-build`는 모든 명령을 실행해 각 결과를 보고하며, ownership 검사는 모든 선언 오류를 보고한다. Red와 Green: 원인마다 사례 하나.
- [ ] S-23-8 각 실패가 원인을 적게 한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: 없음. 원인: `tools/run_tests.py`의 timeout은 test 출력을 버리고, `tools/bench.py`, `tools/verify_engine_status.py`와 workspace 사례 하나는 실패한 명령의 standard error를 숨기며, 여러 검사가 기대 값과 실제 값 없이 실패하고, runtime 사례 둘은 bare timeout을 보고하며, `tools/bench.py`와 archive 사례 하나는 `cargo metadata`에 시간 제한을 둔다. 수용 기준: 그런 실패마다 명령 출력, 기대 값과 실제 값을 출력하고 `cargo metadata`에는 시간 제한이 없다. Red와 Green: 메시지마다 사례 하나.
- [ ] S-23-9 도구 결과를 안정된 출력으로 판정한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-23-3. 원인: ownership과 feature 검사는 nextest의 사람용 출력을 parse하고, engine 상태 검사는 containerctl의 JSON 형태를 읽으며, mount 검사는 filesystem `virtiofs`를 요구하고, browser launch 사례는 browser 오류 문구를 요구하며, archive 사례 하나는 `make --dry-run`의 출력을 읽는다. 수용 기준: nextest 결과는 JUnit report에서 읽고, mount 검사는 접근 mode만 판정하며, browser 사례는 종료 상태와 단계 줄을 판정하고, archive 사례는 Makefile의 recipe를 읽는다. Red와 Green: 검사마다 사례 하나.
- [ ] S-23-10 runtime 취소 사례를 event로 판정한다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: 없음. 원인: `unread_stream_expires_and_releases_worker`는 400 ms를 잠들고 300 ms request timeout이 지났다고 가정하며, `crates/ssr-runtime/tests/cancellation.rs`의 stream 사례는 정상 render에 300 ms timeout을 주므로 부하가 큰 host는 올바른 코드를 실패시킨다. 수용 기준: 만료 사례는 만료 event를 기다리고, 정상 render는 nextest 제한으로만 묶인다. Red와 Green: test 전용 지연으로 host 시계를 늦춰도 사례가 통과한다.
- [ ] S-23-11 React 의존성 사례의 symbolic link를 없앤다. 우선순위: 다음 full suite 실행 전에 필요하다. 의존: S-23-4. 원인: `crates/ssr-adapter-react/tests/react.rs`는 설치된 React package를 의존성 directory에 link하는데, `AGENTS.md`는 이를 허용하지 않는다. 수용 기준: 사례는 package를 복사하거나 link 없이 설치된 directory를 가리킨다. Red: test가 만든 symbolic link에서 검사가 실패한다. Green: 검사가 통과한다.
- [ ] S-24 `ssr`를 GitHub CI와 x86_64 Linux에서 빌드한다. 우선순위: 아래 조건이 유지되는 동안 없다. 의존: S-22. 원인: workspace가 local fork에 의존하므로 GitHub CI와 x86_64 Linux는 빌드할 수 없다. 수용 기준: GitHub CI job과 x86_64 Linux 빌드가 `ssr-server`를 컴파일하고 test한다. 재시도 조건: workspace가 더 이상 local fork에 의존하지 않는다.
