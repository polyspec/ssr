[English](core.md)

# 렌더 계약

`ssr-core`는 Rust 직접 호출과 `POST /_render`가 공유하는 페이지 입력 및 렌더 결과를 정의한다.
JSON 요청 본문은 정확히 다섯 필드가 있는 페이지 객체이며, 인코딩 순서는 `render`, `title`,
`language`, `props`, `state`다. `render`는 `ssr` 또는 `csr`이고, `title`과 `language`는
문자열이며, `props`는 객체이고, `state`는 모든 JSON 값이 가능하다. 모든 필드는 필수다.
추가 필드, 누락 필드, 잘못된 타입, 알 수 없는 렌더 모드와 잘못된 JSON은 오류를 반환한다.
JSON 키가 반복되면 ordered-json이 첫 키 위치와 마지막 값을 유지한다.
`state`를 조용히 null로 바꾸지 않는다. JSON 파싱과 인코딩은 ordered-json을 사용하며
`props`와 `state`의 필드 순서 및 숫자 토큰을 유지한다.

렌더 결과는 HTML 바이트와 JSON 값인 출력 상태를 포함한다. 렌더링 구성 요소가 이 상태를
선택한다. 출력 상태는 입력 상태와 별개이며 브라우저가 hydration에 사용한다. `ssr-core`는
캐시 헤더나 HTTP 상태 코드를 결정하지 않는다. HTTP 서버가 잘못된 요청 및 렌더 실패를
응답으로 변환한다. `CALL_PATH`는 `/_render`다.

완료 기준: [SSR 페이지 fixture](../crates/ssr-core/tests/fixtures/ssr.json)와
[CSR fixture](../crates/ssr-core/tests/fixtures/csr.json)는 파싱, 인코딩, 재파싱 후에도
값과 객체 순서를 유지한다. 잘못된 사례는 오류를 반환한다. 렌더 결과는 HTML과 출력 상태를
유지한다.

ordered-json 의존성은 로컬에서 사용하며 Cargo 레지스트리에 게시하지 않는 패키지다.
의존성 검사는 게시하지 않는 로컬 패키지를 라이선스 판정에서 제외한다. 레지스트리
의존성에는 허용 라이선스 목록을 계속 적용한다.
