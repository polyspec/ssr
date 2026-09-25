[English](checklist.md)

# 체크리스트

상태: `[ ]` 대기, `[~]` 진행 중, `[o]` 완료. 절차는 [AGENTS.ko.md](../AGENTS.ko.md)를 따른다. 각 항목은 의존과
완료 증거를 적는다.

## 요구 사항

- Rust 프로세스 하나가 번들링과 렌더링을 한다. 서버 빌드 시점과 렌더 시점에 Node 프로세스를 실행하지 않는다.
  빌드 전에 `npm ci`가 패키지를 설치한다.
- 렌더 호출은 외부 함수 경계를 넘지 않는다. 호출자는 Rust이며 런타임을 직접 호출한다.
- 번들은 런타임마다 한 번 적재·컴파일하며, 호출마다 보내거나 비교하지 않는다.
- props는 호출마다 한 번 V8 값으로 들어가고, 결과는 바이트 스트림으로 나온다.
- React는 `renderToReadableStream`으로 렌더한다. 스트리밍이 아닌 호출은 같은 스트림을 모은다.
- 컨텍스트 초기화는 전역 상태를 다시 만들지 않고 스냅샷에서 복원한다.

## 항목

- [o] S-0 작성자 `min-median-max`로 로컬 Git 저장소를 초기화하고 `AGENTS.md`, 이 체크리스트, 변경 기록,
  `.gitignore`를 둔다. 의존: 없음. 증거: 첫 커밋, `git remote -v` 출력 없음.
- [ ] S-1 [AGENTS.ko.md](../AGENTS.ko.md)의 크레이트로 Cargo 워크스페이스(`rust-toolchain.toml`: Rust 1.98.1,
  edition 2024)와 Makefile 목표 `check`(rustfmt, clippy `-D warnings`, `cargo deny check`, 기록·용어·문서 검사,
  테스트별 타임아웃을 가진 cargo-nextest 0.9.146 단위 테스트), `bench`를 만든다. 의존: S-0. 증거: `make check`
  종료 코드 0.
- [ ] S-2 `ssr-core`를 구현한다: `POST /_render`의 렌더 호출 `Page{render,title,language,props}`, 결과, 오류,
  캐시 헤더 상수(`public, max-age=60, stale-while-revalidate=300`, `private, no-store`, `Vary: Accept-Language`).
  의존: S-1. 증거: JSON fixture가 ordered-json으로 왕복.
- [ ] S-3 rolldown 1.2.11로 `ssr-build`를 구현한다: TSX·TS에서 서버와 클라이언트 번들, CSS, 내용 해시 파일 이름,
  매니페스트. 의존: S-2. 증거: 샘플 애플리케이션 빌드, 두 번 빌드해도 해시 동일, CSS 파일 산출.
- [ ] S-4 deno_core 0.412.0, deno_webidl 0.259.0, deno_web 0.290.0으로 `ssr-runtime`을 구현한다: 워커 스레드마다
  런타임 하나인 풀, 런타임마다 번들 한 번 컴파일, `console`과 `crypto.getRandomValues`의 자체 연산, fetch·I/O
  타이머·파일·네트워크 없음. 의존: S-2. 증거: 요청 A의 전역 변경이 요청 B에 없음, Web API 사례 통과, 새 번들
  해시의 호출은 제자리 재컴파일 대신 런타임을 교체.
- [ ] S-5 `renderToReadableStream`으로 React 어댑터를 구현한다: 셸이 준비될 때 상태와 헤더를 확정하고, 셸 전 오류는
  오류 페이지와 함께 500으로 응답하고, 셸 후 오류는 스트림 안에 오류 경계를 렌더하고 기록하며, 모든 인라인
  script와 style은 호출의 nonce를 가진다. 의존: S-3, S-4. 증거: 세 사례 통과.
- [ ] S-6 서버 번들 해시, deno_core 버전, 라이브러리 버전을 키로 하는 스냅샷을 구현한다. 컨텍스트 초기화는
  스냅샷에서 복원하고, 키가 다르면 새 스냅샷을 만든다. 의존: S-4. 증거: 불일치 테스트와 초기화 테스트.
- [ ] S-7 `ssr-server`를 구현한다: `POST /_render`, 렌더 시간·풀 대기·힙의 tracing 메트릭, sourcemap 9.3.2로
  역매핑한 스택. 의존: S-5. 증거: HTTP 사례와 역매핑 스택 테스트.
- [ ] S-8 notify 8.2.0으로 개발 모드를 구현한다: 파일이 바뀌면 번들을 다시 만들고 풀을 교체한다. 의존: S-7. 증거:
  파일 변경 후 출력 변경.
- [ ] S-9 Vue, Svelte, vanilla 어댑터와 `features.json`을 구현한다. 의존: S-5. 증거: 모든 기능에 증거.
- [ ] S-10 V8 아카이브를 경로와 SHA-256으로 고정한다. 다운로드 실패는 오류다. 의존: S-4. 증거: 네트워크 없이 빌드
  성공.
- [ ] S-11 `make bench`를 구현한다: 고정 페이지의 초당 렌더 수와 동시 호출 1, 4, 16에서의 p50/p99 지연, 컨텍스트
  초기화의 시간과 메모리를 측정하고 한도를 `docs/benchmarks.md`에 기록한다. 의존: S-6, S-7. 증거: 벤치마크 출력과
  기록된 한도.
