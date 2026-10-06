[English](vanilla.md)

# Vanilla 어댑터

`polyspec-ssr-adapter-vanilla`는 절대 경로의 서버 애플리케이션 모듈 하나에서 서버 진입점을 만들고 절대 경로의
클라이언트 애플리케이션 모듈 하나에서 클라이언트 진입점을 만든다. 서버 모듈은 HTML 문자열을 반환하고
`renderState.output`을 설정하는 `render(props, renderState)`를 export한다. 클라이언트 모듈은
`hydrate(root, props, renderState)`와 `mount(root, props, renderState)`를 export한다. `polyspec-ssr-build`는
생성한 진입점을 build root(`BuildConfig::root`) 아래의 파일로 받는다. 서버 번들 바이트로 `Pool`을
만들고 빌드 매니페스트의 공개 클라이언트·스타일 URL로 `VanillaAdapter`를 만든다.

서버 결과에는 필수인 빈 `head` 문자열이 들어간다. 이 어댑터는 해당 출력의 문서 배치 기준이 없어
비어 있지 않은 `head`를 거부한다. SSR에서는 서버 HTML과 출력 상태를 포함한 문서를 반환한다.
CSR에서는 풀을 호출하지 않고 빈 root와 변경 없는 입력 상태를 반환한다. 두 문서는 같은 클라이언트
URL을 사용한다. 문서는 root 요소에 렌더 모드를 기록한다. 클라이언트 진입점은 빈 HTML을 포함한
SSR에 `hydrate`를, CSR에 `mount`를 호출하며 모드가 없거나 잘못되면 실패한다. DOM 변경과 이벤트 처리기는 애플리케이션이
담당한다. Hydration은 기존 서버 노드를 유지해야 하며, 브라우저 사례는 노드 동일성과 버튼 이벤트를
검증한다.

`static_shell`은 빈 props와 null 상태를 가진 CSR 페이지만 허용한다. 해당 문서는 서로 다른 경로에
제공할 수 있다. 클라이언트와 스타일 URL은 점 경로·질의·fragment가 없는 로컬 절대 JavaScript 또는
CSS 경로여야 한다. 제목과 언어 값은 HTML 이스케이프를 적용한다. 포함한 JSON의 `<`, `>`, `&`를
이스케이프해 입력이 script 요소를 닫지 못하게 한다. 잘못된 UTF-8 서버 HTML은 오류다. 브라우저
사례는 생성한 진입점을 빌드하고 SSR hydration, 빈 SSR, CSR 렌더, 셸 경로 두 개, 잘못된 모드와 JavaScript 오류를 확인한다.
Node는 브라우저만 제어하고 빌드·렌더는 Rust에서 실행한다.
