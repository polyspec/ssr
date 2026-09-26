[English](vue.md)

# Vue 어댑터

`ssr-adapter-vue`는 절대 경로의 JavaScript 모듈에서 기본값으로 export한 Vue 컴포넌트 하나에 대한 서버·
클라이언트 진입점을 만든다. 소비자는 생성한 진입점을 애플리케이션 루트 아래에 기록해 `ssr-build`에
전달한다. 서버 번들 바이트로 `Pool`을 만들고, 빌드 매니페스트의 공개 클라이언트·스타일 URL로
`VueAdapter`를 만든다. Fixture는 Vue 3.5.43을 고정한다. 생성한 서버 진입점은 `createSSRApp`을
사용하고 `vue/server-renderer`의 `renderToString`을 기다린다. 런타임은 V8 microtask를 완료하고
Promise 거부 상세 정보를 반환한다. [Vue SSR API](https://vuejs.org/api/ssr)는 Promise 결과를 정의한다.

컴포넌트는 페이지 props와 입력 상태 및 null로 초기화한 출력을 가진 `renderState`를 받는다.
서버 결과에는 필수인 빈 `head` 문자열이 들어간다. 이 어댑터는 해당 출력의 문서 배치 기준이 없어
비어 있지 않은 `head`를 거부한다. SSR은 렌더한 HTML과 출력 상태를 문서에 담아 반환한다.
CSR은 풀을 호출하지 않고 빈 root와 변경 없는 입력 상태를 반환한다. 두 모드는 같은 클라이언트 URL을
사용한다. 문서는 root 요소에 렌더 모드를 기록한다. 클라이언트 진입점은 빈 본문을 포함한 SSR에
`createSSRApp(...).mount(root)`를 호출하고 CSR에 `createApp(...).mount(root)`를 호출한다.
모드 누락과 잘못된 모드는 실패한다.
[Vue SSR 가이드](https://vuejs.org/guide/scaling-up/ssr)는 hydration mount 동작을 정의한다.
브라우저 사례는 mount 뒤에도 SSR `main` 노드가 같은 노드인지와 버튼 이벤트가 작동하는지 확인한다.
빈 SSR, CSR, 잘못된 모드와 두 경로에서 정적 셸도 확인한다.

`static_shell`은 CSR, 빈 props, null 상태를 요구한다. 클라이언트와 스타일 URL은 점 경로·질의·
fragment가 없는 로컬 절대 JavaScript 또는 CSS 경로여야 한다. 어댑터는 제목·언어에 HTML
이스케이프를 적용하고 포함한 JSON의 `<`, `>`, `&`를 이스케이프한다. 잘못된 UTF-8 서버 HTML은
오류다. Node는 브라우저만 제어하고 빌드·렌더는 Rust에서 실행한다.
