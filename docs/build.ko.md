[English](build.md)

# 빌드 검증

## 제품 빌드 계약

`ssr-build`는 애플리케이션 루트, 서버 TSX 진입점, 클라이언트 TSX 진입점, CSS 진입점의 절대
경로와 공개 자산의 로컬 절대 URL 경로를 받는다. 공개 경로는 `/`이거나 ASCII 문자·숫자·하이픈·
밑줄·마침표로 된 구간을 사용할 수 있다. 빈 구간과 `.`·`..` 구간은 거부한다. 진입점과 CSS는 애플리케이션 루트 내부에 있어야
한다. 빌드는 매니페스트와 출력 경로에서 바이트로 대응되는 자료를 반환하며 게시하지 않는다.
매니페스트 JSON은 ordered-json으로 만들고 서버 진입점, 선택적인 React 프레임워크 진입점, 비공개 서버 소스맵,
클라이언트 진입점, 스타일, 서버 청크,
공개 자산의 경로, 공개 대상의 URL, 콘텐츠 유형과 전체 SHA-256을 식별한다. JavaScript 출력은
Rolldown의 내용 해시 이름을 유지하며 CSS와 CSS URL 자산은 SHA-256 이름을 사용한다. 출력
서버와 클라이언트에서 동일한 자산은 공개 파일 하나를 공유한다. 같은 출력 경로의 바이트가 다르면 오류다.
서버 JavaScript 출력마다 비공개 `.map` 파일이 있다. 빌드는 소스 경로와 매핑 토큰을 일정한 순서로
기록하고, 동일한 빌드 사이에서도 달라질 수 있는 소스 내용은 포함하지 않는다. 소스맵은 스택
역매핑에 필요한 생성 위치와 소스 위치를 유지한다. 매니페스트는 각 소스맵의 SHA-256을 기록하며
공개 파일 선택에서 소스맵을 제외한다.

React의 `react_framework_entry`는 같은 루트 아래의 별도 절대 소스 파일을 지정한다. 이 파일은
`server_entry`와 달라야 한다. 빌드는 비공개 해시 React 프레임워크 번들과 그 비공개 소스맵을 출력한 뒤,
프레임워크 번들이 제공하는 하나의 React 객체를 `react`로 참조하는 IIFE 애플리케이션 서버 번들을
출력한다. 두 번들은 하나의 V8 context에서 실행하며 애플리케이션 모듈 초기화와 렌더에 같은 React
인스턴스를 사용한다. 매니페스트는 프레임워크 경로, 바이트 해시 및 소스맵을 기록하고,
프레임워크 파일에는 공개 URL이 없다. React 애플리케이션 서버 빌드는 코드를
분할하지 않으며 매니페스트에 서버 청크가 없다. 서버 생성 시 React 빌드에 서버 청크가 있으면
그 바이트를 버리지 않고 오류를 반환한다. 일반 서버 빌드는 ECMAScript 모듈을 사용하며
`ServerBundle`로 기록된 서버 청크를 해석한다. 프레임워크 파일이 없거나 서버 진입점과 같으면 실패한다.
React 프레임워크 진입점이 없는 빌드는 단일 서버 번들을 출력하며 매니페스트의 프레임워크 값은 null이다.

`PublicFiles::new`는 공개 URL이 있는 클라이언트 진입점, 스타일과 자산만 선택한다. 게시나 제공 전에
각 URL, 출력 바이트, SHA-256 값과 콘텐츠 타입을 검증한다. `PublicFiles::publish`는 심볼릭 링크가
없는 기존 절대 디렉터리를 요구하고 공개 파일을 URL 경로 아래에 기록한다. 쓰기 전에 다른 내용의 기존
파일이나 일반 파일이 아닌 대상을 거부하며, 같은 내용의 파일은 기존 파일 식별자를 유지한다. 새 파일은
대상 디렉터리의 임시 이름으로 기록하고 동기화한 뒤 기존 파일을 교체하지 않고 최종 이름에 연결한다.
관련 없는 파일은 유지한다. `ssr-server`의 HTTP 제공 함수는 정확한 공개 URL의 GET과 HEAD를 처리하고
빌드 바이트, 콘텐츠 타입, 콘텐츠 길이, SHA-256 ETag, 변경 불가 캐시 헤더를 반환한다. 다른 경로에는
404를, 다른 메서드에는 405를 반환한다. 서버 번들과 서버 청크는 공개하지 않는다.

JavaScript 자산 import에는 Rolldown의 `load`와 `resolve_file_url` 훅을 사용한다. load 훅이
파일을 출력하고 URL 훅이 공개 절대 URL의 JavaScript 문자열 리터럴을 반환한다. 따라서 서버와
클라이언트 코드는 같은 공개 자산 경로를 참조한다. 애플리케이션 루트 밖의 자산과 지원하지 않는
자산 질의·프래그먼트는 거부한다.

추적되는 샘플은 두 번 빌드한 파일과 매니페스트 바이트가 동일해야 한다. 매니페스트의 모든
해시는 해당 바이트와 일치해야 한다. 클라이언트 진입점에는 React 클라이언트 코드, 서버 진입점에는
React 렌더링 코드가 있어야 하며 동적 import는 별도 청크를 출력해야 한다. 나열된 열 가지 이미지와
글꼴의 소스 바이트를 보존해야 한다. 패키지 CSS와 로컬 CSS의 `url()`은 해당 공개 파일을 참조해야
한다. 진입점, import 또는 URL 자산이 누락되면 실패해야 한다.

## 합격 기준

추적되는 샘플은 rolldown 1.2.11의 Rust API로 React TSX의 별도 서버·클라이언트 진입점을 빌드한다.
두 빌드는 내용 해시를 포함하는 JavaScript 이름을 출력한다. 동적 import는 별도 청크를 출력한다.
클라이언트 번들에는 React 코드가, 서버 번들에는 React 렌더링 코드가 포함되어야 한다.

샘플은 lightningcss 1.0.0-alpha.72로 CSS 진입점과 `node_modules`의 패키지 스타일시트를 결합한다.
두 스타일시트의 상대 `url()`은 각각의 소스 파일을 기준으로 해석되고, 출력된 해시 자산을 참조해야 한다.
샘플은 png, svg, jpg, gif, webp, avif, ico, woff, woff2, ttf 파일의 출력 내용과 해시 이름을
검증한다. 해석되지 않는 import나 자산은 빌드 실패여야 한다.

`make verify-build`는 버전이 고정된 샘플 패키지를 설치하고 검증 테스트와 검증 도구의
의존성 검사를 실행한다. 검증 도구는 `tools/build-probe`의 별도 Cargo 워크스페이스에 있다.
해당 CSS 라이브러리는 번들러의 소스 위치와 소스맵 생성을 분리하므로 검증 도구는 그
라이브러리 체크아웃을 사용한다. 검증 도구 매니페스트는 그 체크아웃을 경로로 지정하고,
이 경로는 검증 도구에만 사용한다. 제품 워크스페이스에는 S-5 전까지 빌드 의존성이 없으며 S-5에서
해당 크레이트를 제품 빌드에 연결해야 한다.

## 결과와 API

Rust 1.98.1을 사용하는 macOS aarch64에서 샘플이 통과했다. `Bundler::new(BundlerOptions)`와
`Bundler::generate().await`로 React TSX의 두 진입점을 빌드한다. 각 진입점에 `InputItem`을 지정하고,
`platform`은 `Browser`, `format`은 `Esm`, `code_splitting`은 `Bool(true)`,
`entry_filenames`와 `chunk_filenames`는 `[name]-[hash].js`로 지정한다. `asset_filenames`는
`assets/[name]-[hash][extname]`으로 지정하고 열 가지 확장자를 `ModuleType::Asset`에 대응시킨다.
검증은 진입 청크의 React 클라이언트·서버 렌더링 코드, 동적 import의 별도 청크, 열 가지 자산의
소스 바이트 보존을 확인한다. 누락된 JavaScript 자산의 실패도 확인한다. 자산 샘플은 빌드의
파일 처리를 확인하기 위한 불투명한 바이트이며 이미지나 글꼴 디코딩은 확인하지 않는다.

`lightningcss::bundler::Bundler::new`에는 `node_modules`의 패키지 import를 해석하는
`SourceProvider`를 사용한다. 기본 `FileProvider`는 원천 파일을 기준으로 상대 import만 해석하므로
패키지 import에는 이 제공자가 필요하다. `PrinterOptions::analyze_dependencies`를 지정해
`StyleSheet::to_css`를 호출하면 `url()`의 소스 위치와 치환 표시를 얻는다. 호출자는 각 URL을
해당 스타일시트 기준으로 읽고, SHA-256 이름으로 자산을 출력하고, 표시를 공개 URL로 치환한다.
샘플은 로컬 SVG와 패키지 WOFF2 참조를 확인하고, 누락된 CSS import의 실패를 확인한다.
Lightning CSS는 파일을 출력하거나 치환 표시를 최종 URL로 바꾸지 않으므로 이 명시적 자산
처리가 필요한 대체 방법이다.

`make verify-build`는 Rust 테스트 전에 `npm ci`를 실행한다. 이후 번들링과 CSS 처리는 Node
프로세스 없이 Rust에서 실행된다. 샘플은 브라우저의 이미지·글꼴 디코딩을 검증하지 않으며
자산 바이트는 미디어 샘플이 아니다. 샘플 테스트 두 개와 의존성 검사가 모두 통과한다.

## 의존성 결과

기존 CSS 번들러 API는 소스맵 지원을 필수로 포함하여 rkyv 0.7.46을 통해
[RUSTSEC-2026-0235](https://rustsec.org/advisories/RUSTSEC-2026-0235)에 연결됐다.
CSS 라이브러리는 이제 번들링의 소스맵 지원을 선택적으로 제공한다. 소스 위치는 CSS 모듈
이름과 `url()` 참조에 해당 스타일시트를 계속 제공한다. 검증 도구는 `sourcemap` 없이
`bundler`를 사용하며 잠금 파일에는 parcel_sourcemap과 rkyv가 없다. CSS 라이브러리 테스트는
소스맵 사용 시 120개, 사용하지 않을 때 117개가 통과했다. `make verify-build`의 샘플
테스트 두 개와 cargo-deny 검사도 모두 통과한다. 필요한 Rolldown 의존성의 정확한 버전인
xxhash-rust 0.8.18과 dragonbox_ecma 0.1.12의 라이선스 예외는
[workspace.ko.md](workspace.ko.md)에 기록했다.
검증 도구의 Cargo 빌드 파일을 제거하고 샘플 패키지를 다시 설치한 클린 빌드도 통과했다.

rolldown 1.2.11은 CSS 모듈에 명시적인 미지원 오류를 반환한다. 선택한 CSS 대체 방법은
패키지를 해석하는 `SourceProvider`와 명시적인 자산 URL 치환을 포함한 Lightning CSS
번들러다. 현재 빌드는 CSS 소스맵을 요청하지 않는다. CSS `sourcemap` 기능을 활성화하면
해당 호출자의 의존성 그래프에 `parcel_sourcemap`과 `rkyv`가 추가되므로 사용 전에 그 그래프의
보안 권고를 별도로 검사해야 한다.
