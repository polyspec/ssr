[English](workspace.md)

# 워크스페이스

Rust 1.98.1 워크스페이스는 AGENTS.md에 적힌 여덟 크레이트로 구성한다. 모든 크레이트는 edition 2024와 버전
0.0.1을 쓴다. 워크스페이스 잠금 파일은 의존성 해석 결과를 기록한다. 직접 의존성은 공식 배포 정보를 확인한 뒤
정확한 버전으로 지정한다.

`make check`는 문서 쌍과 링크, 기록, 용어, Python 도구 테스트, Rust 형식, Clippy 경고, 의존성 보안 공지와
라이선스·출처, Rust 단위 테스트를 검사한다. Rust 테스트는 cargo-nextest 0.9.146으로 실행한다. nextest 설정은
각 테스트를 30초 뒤 종료하고 시작 및 결과와 경과 시간을 출력한다. 테스트 도구가 없으면 명령은 실패한다.
`make check`는 테스트가 읽는 build-probe fixture를 설치하므로,
`python3 -m tools.holder_lock run check`가 모든 단계(`make check-steps`) 동안 checkout lock
`var/locks/check.lock`을 잡는다. 같은 checkout의 두 번째 `make check`는 holder의 checkout, pid, process
시작 시각과 함께 거부된다. `make check` 밖의 test 실행은 이 lock을 잡지 않는다.

`make bench`는 Cargo 벤치마크를 실행한다. 유지할 측정과 한도는 S-11에 명시한다.

Rust 1.98.1은 고정한 [Rust 배포](https://github.com/rust-lang/rust/releases/tag/1.98.1)이다.
cargo-nextest 0.9.146은 고정한 [테스트 실행기 배포](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146)다.
의존성 검사는 MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib,
MPL-2.0을 허용한다. 다른 라이선스는 검사에서 실패한다.
[xxhash-rust 0.8.18](https://crates.io/crates/xxhash-rust/0.8.18)의 메타데이터에는 BSL-1.0이
명시되어 있다. [dragonbox_ecma 0.1.12](https://crates.io/crates/dragonbox_ecma/0.1.12)의
메타데이터에는 LLVM 예외가 적용된 Apache-2.0 또는 BSL-1.0이 명시되어 있다. 의존성 검사는
xxhash-rust 0.8.18에만 BSL-1.0을, dragonbox_ecma 0.1.12에만 LLVM 예외가 적용된
Apache-2.0을 허용한다.
