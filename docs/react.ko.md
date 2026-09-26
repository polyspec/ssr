[English](react.md)

# React 어댑터

`ssr-adapter-react`는 절대 경로의 애플리케이션 컴포넌트 하나를 위해 `framework_entry`,
`server_entry`, `client_entry` 소스를 제공한다. 소비자는 이를 애플리케이션 root 아래의
TSX 파일로 쓰고 `react_framework_entry: Some(...)`과 함께 절대 경로를 `ssr-build`에
전달한다. 어댑터는 `ssr-core`와 `ssr-runtime`에만 의존한다. 비공개 프레임워크 번들과
애플리케이션 서버 번들은 한 V8 context에서 실행하며 같은 React 인스턴스를 사용한다.
애플리케이션 서버 번들에는 서버 청크가 없다. 호출자는 프레임워크 경로·바이트와
애플리케이션 경로·바이트를 담은 `ServerBundle`을 `Pool::new_react`에 전달한다.
HTTP 사용에서는 `Server::new`가 매니페스트를 검증하고 이 호출을 수행한다.

React SSR은 SSR 페이지와 요청 nonce를 `Pool::render_stream`에 전달한다. 셸이 준비되면
출력 상태와 스트림을 반환한다. 애플리케이션은 props와 입력 상태 및 null로 초기화한 출력
필드를 가진 `renderState`를 받는다. 출력 필드는 셸 준비 시 확정되며 이후 변경은 오류다.
`ReactAdapter::stream_parts`는 문서 앞부분과 뒷부분을 반환하고 호출자는 그 사이에
스트림을 전송한다. React의 Suspense fallback과 클라이언트 복구 지시를 스트림에 보존한다.
서버는 전체 문서의 모든 인라인 script와 style에 동일한 요청 nonce를 적용한다. React SSR은
`html`이나 `head` 필드를 반환하지 않는다.

`ReactAdapter::render`는 CSR만 처리한다. root를 비워 두고 입력 상태를 변경하지 않고
넣으며 런타임 풀을 호출하지 않는다. 두 모드 모두 같은 클라이언트 URL을 불러온다.
클라이언트는 내용이 있는 root를 hydrate하고 빈 root를 렌더한다. 애플리케이션은 문서에
담긴 상태를 `renderState.input`으로 사용한다.

`ReactAdapter::static_shell`은 빈 props와 null 상태를 가진 CSR `Page`를 받는다. 반환한
문서 하나를 서로 다른 경로에 제공할 수 있으며 애플리케이션은 로드 뒤 브라우저 경로를
읽을 수 있다. 비어 있지 않은 props, null이 아닌 상태 또는 SSR 모드는 오류다.
클라이언트·스타일 URL은 점 경로, 쿼리와 fragment가 없는 로컬 절대 JavaScript·CSS 경로여야
한다. 제목과 언어는 HTML 이스케이프하고, script 요소의 JSON은 `<`, `>`, `&`를
이스케이프하여 입력이 script 요소를 닫지 못하게 한다.

브라우저 테스트는 `npm ci`로 설치한 `playwright-core` 1.63.0과 macOS의
`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` 또는 Linux의
`/usr/bin/google-chrome`에 설치된 Google Chrome을 사용한다. 패키지나 브라우저
실행 파일이 없으면 테스트가 실패한다. Node는 브라우저만 제어하고 Rust 프로세스가
빌드와 렌더를 수행한다. 테스트는 SSR, CSR, script 요소 입력, 두 경로에 있는 동일한
정적 셸을 로드한다. SSR DOM 노드의 hydration 뒤 유지, CSR root 렌더, 클라이언트에
도달한 상태 값, 삽입된 script의 미실행, 브라우저 JavaScript 오류 부재를 확인한다.
별도 브라우저 사례는 Suspense fallback과 React 클라이언트 복구 지시를 받은 뒤 정상 재시도와
재시도 실패 시 애플리케이션 오류 경계를 확인한다.
