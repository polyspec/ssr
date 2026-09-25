[English](build.md)

# 빌드 검증

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
