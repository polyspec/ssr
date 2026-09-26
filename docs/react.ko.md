[English](react.md)

# React 어댑터

`ssr-adapter-react`는 절대 경로에 있는 애플리케이션 컴포넌트 하나의 `server_entry`와
`client_entry` 소스를 제공한다. `ssr-build`는 이 소스를 build root 아래의 TSX 파일로
절대 경로로 받는다. 어댑터는 `ssr-core`와 `ssr-runtime`에만
의존한다. 호출자는 빌드 매니페스트의 공개 클라이언트·스타일 URL을 `ReactAdapter::new`에,
서버 진입점 바이트를 `Pool::new`에 전달한다.

서버 진입점은 props와 입력 상태 및 null로 초기화한 출력 필드를 가진 `renderState`를
애플리케이션에 전달하고 React HTML과 출력 상태, 필수인 빈 `head` 문자열을 반환한다.
문서 배치 기준이 없는 비어 있지 않은 `head`는 어댑터가 거부한다. `ReactAdapter::render`는
SSR 문서에 HTML과 출력 상태를 넣는다. CSR에서는 root를 비워 두고 입력 상태를 변경하지
않고 넣으며 런타임 풀을 호출하지 않는다. 두 모드 모두 같은 클라이언트 URL을 불러온다.
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
