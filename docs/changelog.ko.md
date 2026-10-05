[English](changelog.md)

# 변경 기록

## 0.0.1

- 각 실패가 원인을 적게 한다. 벤치마크는 자기 build가 보고한 실행 파일을
  실행하고, engine 상태, archive, 기록, runtime child 검사는 command 출력과 기대
  값, 실제 값을 적는다.

- runtime 취소 사례를 event로 판정한다. 읽지 않은 stream 사례는 잠드는 대신
  worker가 capacity를 돌려주기를 기다리고, 이 사례들의 정상 render는 더 이상
  300 ms로 묶이지 않는다.

- 결과에서 시간, registry, 고정되지 않은 도구를 없앤다. `make check`는 의존성
  금지 목록, 라이선스, 출처를 검사하고 `make review-advisories`가 보안 공지를
  따로 검토한다. license 사례는 추적되는 잠금 파일을 읽는다.
  rustup은 더 이상 없는 toolchain을 설치하지 않는다. `tools/tool-versions.json`이
  모든 도구를 선언하고 각 진입점이 첫 단계 전에 이를 검사한다.

- test build가 컴파일한 예제 프로그램을 실행한다. nextest setup script가
  `ssr-server`의 어떤 test보다 먼저 `development_process`와 `socket_process`
  예제를 build하고 `SSR_DEVELOPMENT_PROCESS`와 `SSR_SOCKET_PROCESS`로 알리므로,
  test target 하나의 실행이 더 이상 이전 소스로 build된 프로그램을 실행하지 않는다.

- test와 검증 단계의 process와 directory를 회수한다. 시작된 engine stack은
  실패 단계 뒤에도 멈추고, 각 도구 test와 browser script는 끝과 제한에서
  죽는 자기 process group에서 실행되며, Rust test는 assertion이 실패해도
  임시 directory를 지운다.

- `tools/test_local_paths.py`의 engine mount 검사 사례가 CSS checkout을 요구한다.
  mount가 추가된 뒤 이 사례는 실패했다.

- 추적되지 않는 file이 있으면 전체 실행을 거부한다. `make check`와
  `make rerun-failed`의 guard는 추적되지도 무시되지도 않는 file이 있는 동안
  각 file을 적고 거부한다. 단계는 기록되는 tree에 없는 그런 file을 읽을 수
  있기 때문이다. `tools/check.py`는 추적되는 file만 읽고, 무시되지 않는
  추적되지 않는 file마다 실패한다.

- 전체 묶음에서 `ssr-server`를 AArch64 Linux에서 native로 빌드하고 lint하고 test한다.
  `make check-linux`는 모든 소스 checkout을 읽기 전용으로 mount하는 checkout의
  container stack에서 Linux gcc toolchain으로 `ssr-server`의 Clippy, 예제 빌드, 단위,
  `development`, `process` test를 실행한다.

- 체크리스트 항목이 진행 중인 동안 push를 거부한다. 추적되는 pre-push hook
  `.githooks/pre-push`는 `python3 -m tools.push_gate hook`을 실행하고, 이것은
  push되는 각 ref의 tip commit과 작업 트리의 체크리스트를 읽어 진행 중인 각 항목을 ref,
  commit, ID, 제목과 함께 적는다. 모든 make 실행은 `core.hooksPath`를
  `.githooks`로 설정하고, `make hooks`가 설정하고 검사하며, 설정되지 않은 동안
  `tools/check.py`와 `make check`의 guard가 실패한다. workflow job `push-gate`는
  GitHub에서 push된 tip commit과 pull request head를 같은 방식으로 검사한다.

- 빌드가 실행되는 동안 도착한 소스 이벤트에 대해 한 번만 빌드한다. 감독 프로세스는
  쌓인 모든 이벤트에 다시 빌드 한 번으로 답하고 그 동안 도착한 이벤트에 정확히 한 번
  더 빌드하므로, 쓰기 한 번이 이벤트마다 애플리케이션을 빌드하지 않고 버려지는
  이벤트도 없다.

- Linux 소스 감시를 컴파일한다. `inotify` 감시 module이 더 이상 `notify` crate를
  가리지 않으므로 `ssr-server`가 다시 Linux에서 컴파일된다.

- `Development::start`가 반환된 뒤의 모든 소스 변경을 관측한다. macOS에서 소스
  감시는 제외 경로를 건너뛰는 walk로 감시하는 디렉터리와 일반 file을 `kqueue(2)`에
  등록하고, 필요한 file descriptor limit을 올리거나 그 수, limit, 가장 큰 디렉터리를
  적어 실패하며, 새 항목을 알리기 전에 등록한다. `start`는 자신이 감시하는
  디렉터리의 sentinel file event 뒤에만 빌드하므로 앞선 쓰기의 event가 더 이상 두
  번째 빌드를 시작하지 않는다.

- 소스 감시가 버린 이벤트를 알리면 다시 빌드한다. 파일 이벤트 서비스가 이벤트를
  버렸을 때 감시가 보내는 `Rescan` 이벤트는 모든 경로가 제외 경로가 아니면 재빌드와
  경고를 시작한다.

- 전체 묶음은 활성 체크리스트 항목이 모두 완료된 뒤 한 번 실행한다.
  `var/full-run.json`이 그 결과와 단계별 시간을 기록하고, 실패는 새 체크리스트
  항목이 된다. 개발 중에는 변경을 소유한 Red·Green 테스트만 실행한다.

- 감독되는 렌더러 요청에 비공개 Unix 소켓을 사용한다. 감독 프로세스는 렌더 프로세스를 시작하기
  전에 소켓 디렉터리를 만들고 소유하며, 렌더 명령에 소켓 경로를 전달하고, 프로세스 종료를 수집한 뒤
  자신의 디렉터리만 제거한다. 준비는 더 이상 서비스 주소를 해석하지 않는다. 감독 프로세스는 Unix
  소켓 주소보다 긴 소켓 경로를 소켓 디렉터리를 만들기 전에 거부하고, 준비는 Unix 소켓이 아닌
  선언 경로와 다른 사용자에게 열린 소켓 디렉터리를 서로 다른 오류로 알린다. development
  사례는 프로세스 종료 뒤 모든 소켓 디렉터리가 제거되고 버린 응답이 자식 응답을 취소하는지 요구한다.

- 체크리스트에는 항목과 제목만 둔다. `tools/check.py`는 빈 줄, 제목, 항목 줄, 항목의
  이어지는 줄이 아닌 체크리스트 줄에 대해 실패한다. 요구 사항은 `docs/requirements.md`로
  옮겼고, `AGENTS.md`가 체크리스트 형식을 적는다.

- GitHub의 task list 상태도 상태 표시로 다룬다. `tools/check.py`는 체크리스트에서
  항목 줄의 상태가 아닌 대괄호 안의 x나 대문자 X에 대해서도 실패한다.

- 체크리스트에서 상태 표시는 항목의 상태로만 쓴다. `tools/check.py`는
  `docs/checklist.md`와 `docs/checklist.ko.md`의 다른 대괄호 상태 표시에 대해 실패하고
  그 file, 줄, 열을 적는다. 체크리스트에는 더 이상 범례가 없다. `AGENTS.ko.md`가 상태를
  정의하고, 문장은 상태를 말로 적는다.

- 실행할 수 없는 전체 묶음을 어떤 단계보다 먼저 거부한다. `make check`는
  `tools/full_run.py`를 거쳐 실행되고, 체크리스트 항목이 `[~]`인 동안(각 ID와 제목을
  나열한다), 추적하는 file에 commit하지 않은 변경이 있는 동안, `var/full-run.json`이 같은
  tree의 전체 실행을 기록하고 있을 때 거부된다. 기록은 첫 단계 전과 각 단계마다 쓰이므로
  강제 종료된 실행은 `incomplete`로 남는다. `make rerun-failed`는 현재 tree의 기록에서
  통과하지 못한 target만 실행한다. target 없는 `make`는 `make check`를 실행한다.

- `make check`의 단계를 checkout lock `var/locks/check.lock` 아래에서 실행하므로,
  같은 checkout의 두 번째 `make check`가 첫 실행의 test가 읽는 동안
  build-probe fixture를 다시 설치하지 않는다. 두 번째 실행은 holder의 checkout, pid,
  process 시작 시각과 함께 거부된다.

- checkout마다 engine 검증 stack을 따로 둔다. Compose project, container,
  image 이름은 checkout path의 SHA-256에서 만들므로, 다른 checkout의 검증이
  실행 중인 stack을 더 이상 바꾸지 않는다. `make verify-engine-linux-arm64`와
  `make verify-engine-down`은 checkout lock `var/locks/engine-verification.lock`
  아래에서 `tools/verify_engine_linux.py`를 실행한다. 두 번째 실행은 holder의
  checkout, pid, process 시작 시각과 함께 거부되고, 각 단계는 결과와 경과
  시간을 시간 제한 없이 출력한다.

- 소유 검사의 워크스페이스 metadata를 시간 제한 없이 읽고, 경과 시간과 함께
  한 단계로 보고하며 종료 코드로 판정한다.

- browser 사례의 browser 시작에 시간 한도를 두지 않는다. 시작과 종료는 경과
  시간과 함께 보고되고 결과로 판정하며, page 단계는 60 s 한도를 유지한다.

- 소유 사례 전에 워크스페이스 테스트를 한 번 빌드하고, 모든 사례를 그 빌드에서
  nextest 실행 한 번으로 실행한다.

- render benchmark를 위해 render CPU time을 워커 thread에서 측정해, 동시 호출이
  더 이상 호출의 CPU time에 더해지지 않는다.

- 각각은 timeout 안에 들지만 합치면 넘는 queue 대기와 실행으로 request deadline
  하나가 queue 대기와 실행을 함께 덮는지 증명한다.

- browser 사례를 page event와 process 종료로 판정하고, 각 browser 단계를 경과
  시간과 함께 출력하며, 시간 한도는 멈춘 사례를 감지하는 데만 쓴다.

- render benchmark를 process CPU time으로 판정한다. CPU 초당 render 수와 CPU p99에
  한도를 두고, wall-clock 값과 context 초기화 시간은 보고만 한다.

- queue에 들어간 호출의 cancellation과 timeout 사례를 wall-clock 한도 대신
  테스트 전용 queue 진입 신호와 반환된 오류 종류로 판정한다.

- 커밋 전에는 변경을 소유한 Red·Green 테스트와 `tools/check.py`만 실행하고,
  전체 묶음은 활성 체크리스트 항목이 모두 끝났을 때 한 번 실행한다고 적는다.

- 끝난 React stream render의 scheduler를 해제해 끝난 모든 render의 context가
  회수되고 반복 render가 heap을 일정 범위 안에 유지한다.

- 각 로컬 V8, CSS, JSON 체크아웃 경로를 루트 매니페스트에 한 번만 선언한다.
  engine Compose 준비와 마운트 검사는 그 경로를 읽고, 추적 사례는 어느
  매니페스트에서든 존재하지 않거나 다른 경로를 거부한다.

- 표준 resolver로 설정된 JavaScript import를 해결하여 범위가 지정된 조건부 exports가
  선언된 입력을 선택하고 export되지 않은 하위 경로는 실패하도록 한다.
  단일 의존성 디렉터리를 유지하고 중첩된 의존성의 대체를 거부한다.

- 모든 애플리케이션 패키지 import를 하나의 설정된 의존성 디렉터리에서 해석한다: 베어 패키지
  지정자는 그 디렉터리에서만 해석되므로 같은 패키지의 중첩 `node_modules` 사본이 렌더 context
  provider와 consumer 옆에 두 번째 모듈 인스턴스를 만들 수 없다.

- 엔진 검증 이미지 안에서 공식 Linux V8 archive와 binding을 검증한다: digest 확인,
  ordered-json을 포함한 읽기 전용 소스 마운트, V8 소스 컴파일 없는 네이티브 동일 스냅샷 병렬 렌더링.

- 닫히거나 시간이 지난 스트림을 취소하고 네이티브 청크 전송을 제한하며 정리 완료 뒤 워커를
  재사용한다. 풀 바이트·힙 한도를 명시하고 사용 불가 워커를 종료 이벤트로 보고하며 스레드
  정리 오류를 반환한다.

- 공식 로컬 V8 archive와 일치하는 binding으로 렌더 프로세스마다 불변 앱 스냅샷 하나를 사용한다.
  공개 스냅샷 생성이 완료된 뒤 동시 워커가 같은 blob을 복원한다. 프로세스 하나가 초기화된 앱 하나를
  유지하므로 다른 번들 키는 별도 렌더 프로세스가 필요하다. 검증은 입력 누락·해시 변경·feature
  불일치를 거부한다. 공식 archive로 네이티브 병렬 복원, React 스트림, 별도 앱 프로세스와 기존
  성능 한도를 검사한다.
- 스냅샷 초기화 중과 스냅샷 성공 뒤의 Svelte 컴파일러 진입을 거부한다. 공통 프로세스 계약은 V8
  진입부터 isolate 정리까지 보호한다. 완료한 컴파일 뒤의 렌더와 일반 Rust 번들링은 계속 허용한다.
  Svelte 변환은 컴파일러 오류 원인을 보존한다. 테스트 소유 검사는 실제 동작 사용과 해당 소비 테스트
  선언을 요구한다.

- 빌드와 렌더 명령을 별도 프로세스에서 실행하며 요청 제공 전에 완료 빌드 디렉터리와 실제 SSR
  준비를 검증한다. 교체 중 응답 프로세스를 유지하고 종료된 렌더 프로세스는 완료 빌드에서 재시작한다.
  재빌드·종료 실패를 보고하고 자식 종료 상태를 수집하며 명시적 출력 제외와 함께 선언한 소스를 감시한다.

- 매니페스트 SHA-256 이름의 전체 빌드 디렉터리를 위한 `Build::write`와 `Build::read`를 추가한다.
  디렉터리 이름 변경 한 번으로 비공개·공개 파일을 게시하고 같은 쓰기와 이전 빌드를 유지한다.
  변조된 매니페스트, 누락·추가 파일, 잘못된 경로, 심볼릭 링크와 잘못된 파일 해시를 거부하며
  게시 실패와 정리 실패를 함께 보고한다.

- 개발 이미지 렌더링에 독립 isolate group을 지원하는 대상별 Linux V8 archive를 요구한다.

- 설정한 dependency 디렉터리에서 application package를 해석해 중첩 package 복사본이 별도 module instance를 만들지 않게 한다.

- Vue, Svelte, vanilla 어댑터를 완성한다. `features.json`은 지원 기능마다 정확한 실행 사례를 지정하며
  `make check`는 선언한 모든 사례를 실행한다.
- 지원하는 어댑터 렌더링, 공개 자산, 소스맵, React 폼 기능을 `features.json`에 기록한다.
  선언한 기능마다 실행 가능한 정확한 사례 하나를 지정한다. 기능 검사기는 모든 사례를 실행하며
  증거 누락·기능 중복 선언, 테스트 부재, 명령의 실패 코드와 시간 초과를 오류로 처리한다. 선언이나 소스
  검사만으로 기능 지원을 확인할 수 없기 때문이다.
- 공유 페이지·빌드 동작의 테스트 소유 위치를 강제한다. 소유 크레이트와 모든 직접 소비 크레이트가
  특정 통합 테스트 함수를 선언하고 `make check`가 각 사례를 실행한다. 누락, 잘못된 위치, 빈 테스트,
  실패, 무시된 테스트와 시간 초과는 오류다. 파일 이름만으로는 동작을 사용하는 크레이트의 검증을
  입증할 수 없기 때문이다.
- HTTP 셸 상태, 늦은 본문 오류, 요청 nonce, 타이머 격리, 렌더 지표 및 Suspense 오류 뒤
  브라우저 복구 사례로 React 스트리밍을 검증한다.
- React 스트림 reader가 본문 청크 뒤에 거부되면 해당 청크와 명시적 본문 오류를 반환하고
  초기 HTTP 상태와 헤더를 유지하는지 검증한다.
- React 어댑터와 런타임 문서에 프레임워크·애플리케이션 진입점, `Pool::new_react`,
  `Pool::render_stream`, `ReactAdapter::stream_parts`를 정확히 기록한다. 이전 React 풀 호출은
  다른 렌더 경로를 설명하므로 기록 검사에서 거부한다.
- React SSR 문서를 셸 준비 후 스트리밍한다. 프레임워크와 애플리케이션을 한 요청 context에서 복원하고
  애플리케이션 전역에는 작업 예약 함수를 두지 않으며, React Suspense 대체 내용과 클라이언트 복구 지시를
  보존한다. 요청마다 서로 다른 운영체제 난수 nonce를 생성해 출력 청크에 걸친 인라인 script와 style에
  적용하고, React 오류를 해당 nonce와 함께 기록한다. React 애플리케이션 번들은 IIFE이므로 서버 청크를
  거부한다.
- 실행 가능한 V8 fixture에서 React 번들 실행 경계를 검증한다. 최상위 React 클래스와
  context가 같은 React 인스턴스를 사용하며, 프레임워크 작업 예약은 함수 인자에 유지하고
  애플리케이션 전역에는 타이머를 두지 않는다.
- 절차만 기록한 S-0-2-1을 체크리스트에서 제거하고 우선순위와 테스트 위치 규칙은 AGENTS.md에
  둔다. 체크리스트 항목에는 저장소 산출물과 검증 가능한 완료 증거를 요구한다.
- Rust 빌드 프로세스 안의 Svelte 소스 컴파일과 Svelte 어댑터를 추가한다. 빌드는 컴포넌트 CSS를
  내용 해시 URL로 게시하고 어댑터는 서버 head 출력을 문서에 배치한다. HTTP 서버는 ESM 빌드와
  컴포넌트 CSS를 제공하고 head와 본문의 인라인 내용에 요청 nonce를 적용하며 JavaScript 오류를
  빌드 소스맵으로 매핑한다. 생성한 번들만으로 브라우저 동작을 확인할 수 없으므로 hydration,
  클라이언트 이벤트, head 출력, 컴포넌트 스타일, CSR과 정적 셸을 브라우저 사례로 검증한다.
- 각 문서에 기록한 렌더 모드로 React, Vue, Svelte, vanilla 클라이언트 hydration을 선택한다. 빈 SSR 본문도
  hydration을 선택하고 CSR은 새 렌더를 선택한다. 본문 내용만으로 요청한 렌더 연산을 식별할 수 없으므로
  모드가 없거나 잘못되면 실패한다.
- 추적되는 fixture `FormRowsApp.tsx`의 반복 행 React 폼을 서버에서 두 번 렌더해 검증한다. 요청마다
  독립된 행 식별자를 만들어야 하므로 중첩 `rows.<key>.<field>` 이름, `row-`와 16진수 여덟 자리인
  행 키와 다음 렌더의 새 키를 확인한다.

- 모든 런타임 렌더 결과에 `head` 문자열을 요구하고 그 바이트를 보존한다. 렌더 출력이 완전해야 하므로
  누락되거나 잘못된 값은 실패한다. React, Vue, vanilla는 빈 head를 반환하고 문서 배치 기준이 없는
  비어 있지 않은 head 출력을 거부한다.

- Promise 기반 서버 렌더와 브라우저 hydration을 지원하는 Vue 어댑터를 추가한다. 생성한 번들만으로
  hydration 동작을 확인할 수 없으므로 브라우저에서 기존 서버 DOM 노드와 연결된 이벤트 처리를 검증한다.

- 서버·클라이언트 진입점, SSR 출력 상태, CSR 문서와 정적 셸을 지원하는 vanilla 어댑터를 추가한다.
  생성한 소스만으로 hydration 동작을 확인할 수 없으므로 브라우저에서 서버 DOM 노드 동일성과 클라이언트
  이벤트 처리를 검증한다.

- 개발 절차(`AGENTS.md`), 체크리스트, 이 변경 기록을 추가한다.
- 스레드 시작 신호만으로는 어느 렌더가 워커를 점유했는지 확정할 수 없으므로 풀 대기·실행을 합친 시간
  초과 사례를 대기열 진입에 동기화한다.
- ssr의 렌더·빌드·엔진 요구 사항과 빌드·엔진 검증을 체크리스트에 추가한다.
- 여덟 크레이트로 Rust 1.98.1 워크스페이스와 check·bench 명령을 만든다. 이 요구 사항이 모든 크레이트에
  적용되므로 check 명령은 기록, 용어, 문서, 의존성, 단위 테스트를 검사한다.
- 설치된 패키지는 관리되는 소스 밖의 의존성이므로 문서·용어 검사에서 해당 패키지 문서를 제외한다.
- 엔진 의존성 버전과 V8 아카이브 해시를 검증한다. macOS arm64, macOS x86_64, Linux x86_64, Linux
  AArch64의 전체 빌드가 링크된다. Linux AArch64 프로그램은 네이티브 컨테이너에서 실행된다.
- 검증한 로컬 V8 아카이브로 Cargo 네트워크 접근을 차단한 상태에서 빌드한다. 아카이브 누락·변조와
  다운로드 실패는 오류를 반환한다.
- 엔진 검증의 Cargo 캐시와 빌드 결과를 무시된 호스트 디렉터리에 바인드 마운트한다. 소스는 읽기 전용으로
  유지하며 엔진 컨테이너를 교체한 뒤에도 링크된 프로그램을 재사용한다.
- 로컬 V8 소스 체크아웃에서 관리 중인 컴파일 시 식별자 매크로 패키지를 선택한다. 이전
  매크로 패키지에는 관리 중인 릴리스가 없으므로 엔진을 네 대상에서 다시 빌드·링크하고,
  테스트 컨테이너에 체크아웃을 읽기 전용으로 마운트하며, 컨테이너 교체 후에도 빌드 캐시를 유지한다.
- 빌드 의존성에 MIT 외의 라이선스가 있으므로 cargo-deny에서 지정된 라이선스를 허용한다.
  지정되지 않은 라이선스는 계속 오류다.
- 빌드가 두 크레이트의 정확한 버전에 의존하므로 xxhash-rust 0.8.18의 BSL-1.0과
  dragonbox_ecma 0.1.12의 LLVM 예외가 적용된 Apache-2.0을 허용한다.
- Rust API로 React TSX 서버·클라이언트 번들, CSS import와 해시 자산 출력을 검증한다.
  패키지 CSS 해석과 URL 치환에는 명시적인 소스·자산 처리가 필요하다.
- `ssr-core`에 페이지 JSON 계약과 렌더 결과를 정의한다. ordered-json이 객체 순서와 숫자 토큰을
  유지하고, 잘못된 필드와 입력은 오류를 반환한다.
- JSON 패키지를 로컬 소스 체크아웃에서 사용하므로 게시하지 않는 로컬 패키지를 라이선스
  검사에서 제외한다. 레지스트리 의존성에는 라이선스 정책을 계속 적용한다.
- 마지막 값만 유지하면 요청 입력이 제거되므로 페이지 JSON의 모든 깊이에서 해석된 객체 키의
  반복을 거부한다.
- `ssr-build`에서 React TSX 서버·클라이언트 번들, CSS, 해시 자산을 빌드한다. 빌드는 전체 파일과
  ordered-json 매니페스트를 반환한다. 청크 기준 상대 URL은 페이지 문서에서 파일을 식별하지 못하므로
  자산 URL 훅이 두 번들에 공개 절대 URL을 제공한다.
- 제한된 `ssr-runtime` 워커 풀에서 서버 번들을 실행한다. 워커마다 isolate를 소유하고,
  요청마다 전역 상태를 초기화하며, 제한 시간에 스크립트를 종료하고, console 출력과
  운영체제 난수를 제공한다. 출력을 조용히 누락하거나 변경하지 않기 위해 잘못된 결과와 사용할 수 없는
  Web API는 명시적 오류를 반환한다.
- 번들 해시와 고정된 버전을 키로 하는 V8 스냅샷에서 서버 전역을 초기화한다. 요청마다 서버 전역을 다시
  구성하면 초기화가 반복되므로 각 요청의 context를 스냅샷에서 복원한다. 키가 달라지면 새 스냅샷을
  만들고 사용하지 않는 스냅샷 바이트를 해제한다.
- 서버 스냅샷마다 독립적인 V8 isolate group을 만들고 워커 isolate를 해당 group에서 복원한다.
  release 소스 빌드가 별도 포인터 cage와 외부 코드 공간을 제공해 서로 다른 애플리케이션 스냅샷을 동시에
  복원할 수 있으며, group 기능을 사용할 수 없으면 실패한다.
  번들 초기화는 설정한 제한 시간에 오류를 반환한다.
- 별도 로컬 V8 소스 archive와 생성된 binding의 SHA-256을 검증한 뒤 일상 검사와 벤치마크에 연결한다.
  GN을 반복 생성하면 archive가 다시 빌드될 수 있으므로 직접 소스 검증은 명시적으로 실행한다.
- 비공개 서버 진입점과 청크를 ECMAScript 모듈로 평가한다. 생성한 진입점은 전역 속성 없이
  `render`를 export하고 상대 import는 제공된 서버 파일만 대상으로 해석한다. 요청 context마다
  모듈 데이터를 복원하고 각 파일의 JavaScript 스택을 해당 소스맵으로 변환한다. 파일이 없거나
  평가가 끝나지 않으면 렌더 코드가 완전하지 않으므로 실패한다.
- React 서버 번들이 필요로 하므로 모든 렌더 context에 UTF-8 `TextEncoder`를 제공한다.
  인코딩과 길이가 제한된 `encodeInto`는 비ASCII 문자와 단독 서로게이트를 처리하며, 잘못된 호출은
  파일·네트워크·타이머 연산을 추가하지 않고 실패한다.
- React 어댑터 진입점과 문서 렌더러를 만든다. SSR은 React HTML과 출력 상태를 반환하고,
  CSR은 입력 상태를 변경하지 않고 빈 root를 반환하며 정적 셸 하나를 여러 경로에 제공할 수 있다.
  JSON을 이스케이프하여 요청이 script 요소를 닫지 못하게 한다. 번들 검사로 DOM 동작을
  확인할 수 없으므로 브라우저에서 hydration, CSR 렌더와 셸 재사용을 검증한다.
- 검증된 공개 빌드 파일을 절대 디렉터리에 게시한다. 게시한 URL의 바이트가 일정해야 하므로 같은 파일은
  유지하고 변경된 파일은 거부한다. 정확한 공개 URL을 콘텐츠 타입과 해시 값과 함께 제공하며 서버 출력은
  공개하지 않는다.
- 비공개 서버 소스맵을 일정한 바이트로 빌드 매니페스트에 기록한다. 서버는 소스맵과 서버 번들을
  검증하고 `POST /_render`를 렌더하며 HTTP 실패를 명시적으로 반환한다. 렌더 시간, 풀 대기 시간,
  V8 힙 사용량을 tracing에 기록한다. 생성된 번들의 위치는 애플리케이션의 소스 줄을 식별하지
  못하므로 JavaScript 실패는 역매핑한 스택 위치를 보고한다.
- 개발 서버는 파일 이벤트 이후 번들을 다시 만들고 서버와 렌더 풀을 함께 교체한다.
  잘못된 변경은 원인과 함께 요청을 실패시키고 올바른 변경 이후 서비스를 복구한다.
  이전 출력을 제공하면 빌드 실패를 숨기기 때문이다.
- `make bench`에서 고정 SSR 페이지를 동시 호출로 측정한다. 명령은 처리량, 호출 지연, 컨텍스트 생성
  시간과 V8 힙 변화를 보고하며 측정 한도를 초과하면 실패한다.
- Promise 렌더 결과를 읽기 전에 V8 microtask를 완료한다. 이행한 결과에는 일반 결과 계약을
  적용하고, 거부는 메시지와 stack을 반환하며, 예정된 완료가 없는 결과는 실패한다. 프레임워크의
  작업 예약은 비공개여야 하므로 별도 context의 React 테스트는 프레임워크 타이머를 받지 않은
  애플리케이션 컴포넌트가 렌더되는지 확인한다.
- 서로 다른 React 인스턴스는 애플리케이션 hook을 거부하므로 하나의 React 인스턴스를 공유하는
  비공개 프레임워크 번들과 애플리케이션 서버 번들을 빌드한다. 빌드는 프레임워크 입력을 검증하고
  해시와 소스맵을 기록하며 공개 파일에서 제외한다. 테스트는 프레임워크 타이머가 없는 context에서
  `useId`를 사용하는 컴포넌트를 렌더한다.
- React 진입점 빌드 테스트와 브라우저 hydration 테스트가 생성한 서버·프레임워크·클라이언트
  파일을 별도 디렉터리에 둔다. 공유 경로가 병렬 검사 중 빌드 입력을 바꿀 수 있으므로 동시
  테스트에서 각 테스트가 자신의 파일을 읽는지 확인한다.
- React CSS import를 명시적인 JavaScript 없는 모듈로 처리하고 스타일시트 번들에서 data URL을 보존한다.
