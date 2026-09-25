[English](workspace.md)

# 워크스페이스

Rust 1.98.1 워크스페이스는 AGENTS.md에 적힌 여덟 크레이트로 구성한다. 모든 크레이트는 edition 2024와 버전
0.0.1을 쓴다. 워크스페이스 잠금 파일은 의존성 해석 결과를 기록한다. 직접 의존성은 공식 배포 정보를 확인한 뒤
정확한 버전으로 지정한다.

`make check`는 문서 쌍과 링크, 기록, 용어, Python 도구 테스트, Rust 형식, Clippy 경고, 의존성 보안 공지와
라이선스·출처, Rust 단위 테스트를 검사한다. Rust 테스트는 cargo-nextest 0.9.146으로 실행한다. nextest 설정은
각 테스트를 30초 뒤 종료하고 시작 및 결과와 경과 시간을 출력한다. 테스트 도구가 없으면 명령은 실패한다.

`make bench`는 Cargo 벤치마크를 실행한다. 유지할 측정과 한도는 S-11에 명시한다.

Rust 1.98.1은 고정한 [Rust 배포](https://github.com/rust-lang/rust/releases/tag/1.98.1)이다.
cargo-nextest 0.9.146은 고정한 [테스트 실행기 배포](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146)다.
