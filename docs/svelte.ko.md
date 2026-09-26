[English](svelte.md)

# Svelte 어댑터

`ssr-build`는 고정된 Svelte 5.57.1 컴파일러를 사용해 각 `.svelte` 소스를 Rust 프로세스 안에서
서버·클라이언트용으로 컴파일한다. 패키지는 빌드 전에 `npm ci`로 설치하며, 빌드는 build root(`BuildConfig::root`)
아래의 서버·클라이언트·CSS 진입점 절대 경로를 받는다. 컴파일에 Node 프로세스는 필요하지 않다.
컴파일러 경고와 오류는 빌드 실패다. 컴파일러 소스맵을 JavaScript 번들러에 전달하고, 생성한 컴포넌트
CSS를 Lightning CSS로 처리해 내용 해시가 있는 공개 URL로 빌드 매니페스트에 기록한다. 서버와
클라이언트 컴파일 결과의 컴포넌트 CSS가 같아야 한다. 컴파일러 출력 누락이나 결과 불일치는 오류다.
Svelte의 AST와 metadata는 컴파일 도구 자료다. 원래 CSS 소스맵은 변환한 CSS의 위치를 표시하지
않으므로 게시하지 않으며 JavaScript 소스맵은 번들러에 전달한다.

`ssr-adapter-svelte`는 절대 경로의 `.svelte` 컴포넌트 하나에 대한 진입점을 만든다. 서버 진입점은
`render` 함수를 export하고 페이지 props와 `renderState`를 전달해 `svelte/server`의 `render`를 호출한다.
호출자는 빌드 매니페스트의 서버 진입점 경로·바이트와 모든 비공개 서버 청크 경로·바이트로
`ServerBundle`을 구성한 뒤 풀을 만든다. 렌더 함수는 본문, head와 출력 상태를 반환한다.
어댑터는 head 출력을 문서 head에, 본문 출력을 root에 넣는다. CSR은 런타임 풀을
호출하지 않고 빈 root와 변경 없는 입력 상태를 반환한다. `Adapter::Svelte`는 HTTP 렌더 경로와 ESM 풀을
사용한다. HTTP 출력은 head와 본문의 인라인 script·style에 요청 nonce 하나를 적용하고, 공개 경로는
빌드 매니페스트의 컴포넌트 CSS를 제공한다. 두 모드는 같은 클라이언트 URL을 사용한다.
문서는 root 요소에 렌더 모드를 기록한다. 클라이언트는 빈 본문을 포함한 SSR에 `hydrate`를,
CSR에 `mount`를 호출하고 모드가 없거나 잘못되면 실패한다.
[Svelte 서버 API](https://svelte.dev/docs/svelte/svelte-server)는 본문과 head 출력을 정의하고,
[Svelte 클라이언트 API](https://svelte.dev/docs/svelte/svelte)는 hydration과 mount를 정의한다.

`static_shell`은 CSR, 빈 props와 null 상태를 요구한다. 클라이언트와 스타일 URL은 점 경로·질의·
fragment가 없는 로컬 절대 JavaScript 또는 CSS 경로여야 한다. 어댑터는 제목과 언어에 HTML
이스케이프를 적용하고 포함한 JSON의 `<`, `>`, `&`를 이스케이프한다. 잘못된 UTF-8 서버 본문이나
head는 오류다. 브라우저 사례는 hydration 후 기존 서버 DOM 노드, 빈 SSR 본문, 버튼 이벤트,
출력 상태, CSR과 셸 페이지, 잘못된 모드, `<svelte:head>` meta 요소 및 게시된 컴포넌트 CSS의
계산된 색상을 검증한다. Svelte는 빈 컴포넌트에도 hydration 주석 마커를 출력한다. 브라우저 사례는
자식 요소가 없는 컴포넌트의 hydration을 확인한다. 마커를 제거해 HTML 본문을 비우면 hydration을
호출한 뒤 hydration 불일치 오류를 보고하며 mount를 선택하지 않는다. Node는
브라우저만 제어하며 빌드와 렌더는 Rust에서 실행한다.
