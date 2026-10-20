<!-- doc-id: checklist-kit-standard -->
<!-- source-sha256: d4faf73c28b4f9633a1c4ed46bacae0a6a5dd631afc24022fe67c01f6de863d9 -->
# Kit 표준 체크리스트

| ID | Task | Deliverables | Verification | State |
|---|---|---|---|---|
| S-58 | kit 의 계획 폴더를 채택한다: 체크리스트를 표로 docs/plans/execution/ 과 docs/plans/kit-standard/ 로 옮기고, config/checklist.json 에 ID 패턴을 선언하며, 변경 기록을 루트의 CHANGELOG.md 와 CHANGELOG.ko.md 로 옮긴다 | docs/plans/execution/, docs/plans/kit-standard/, config/checklist.json, CHANGELOG.md, CHANGELOG.ko.md | `make docs-check` | [o] |
| S-59 | config/php-lint 의 PHP 하한을 플랫폼 PHP 8.4 로 맞춘다 (kit-sync 가 쓰는 config/php-lint/composer.json 과 composer.lock) | config/php-lint/composer.json, config/php-lint/composer.lock | `make toolchain-check` | [o] |
| S-60 | make kit-sync 로 kit 0.0.54 의 복사본을 받는다: lint 설정, kit layout, PRINCIPLES, release 와 gates workflow, bootstrap, .claude/settings.json | the copies named by kit.copies of @polyspec/kit | `make kit-check` | [o] |
| S-61 | workflow test 가 release.yml 과 함께 kit 의 gates workflow 를 kit 복사본으로 받아들이게 한다 | tests/tools/workflows.test.mts | `make tool-test-workflows` | [o] |
| S-62 | config/owner-checks.json 이 kit 0.0.54 의 복사본과 문서 경로를 소유하게 한다: documents-check 대신 docs-check, gates.yml 과 .claude/settings.json 은 kit-check 가 소유한다 | config/owner-checks.json | `make owner-validate` | [o] |
| S-63 | Makefile 의 CHECK_TARGETS 가 kit-gates 의 모든 검사를 실행하게 한다: documents-check 대신 docs-check 를 쓰고 rules-check, rebuild-check, language-check, test-markers-check, duplication-check, parity-declarations-check, parity-check, commits-check, link-state-check, lint-ts 를 추가한다 | Makefile | `make kit-check`, `make tool-test-workflows` | [o] |
| S-64 | make hooks 가 쓰는 kit 0.0.54 의 pre-commit hook 을 pre-push 와 함께 추적한다 | .githooks/ | `make hooks-check` | [o] |
| S-65 | scripts/ 의 도구 소스 7개를 config/rebuild.json 에서 분류한다 (S1): ecosystem, 또는 지키는 규칙을 적은 policy | config/rebuild.json, config/owner-checks.json | `make rebuild-check` | [o] |
| S-66 | record 검사가 계획 폴더의 체크리스트를 대체된 실행 체크리스트처럼 읽게 하고, S-58 부터 S-65 의 기록에서 record word 를 없앤다 | scripts/check-records.mts, tests/tools/check-records.test.mts, CHANGELOG.md, CHANGELOG.ko.md, docs/plans/kit-standard/ | `make tool-test-check-records`, `make check-records` | [o] |
| S-67 | docs/ 의 문서를 kit layout (K245) 에 따라 docs/spec/ (계약과 요구사항) 과 docs/operations/ (workspace, test ownership, benchmarks) 로 옮기고 모든 참조를 고친다 | docs/spec/, docs/operations/, references in AGENTS.md, Makefile, config/, tests/, scripts/check-records.mts | `make layout-check`, `make docs-check`, `make tool-test-check-records` | [o] |
| S-68 | tests/tools 의 도구 테스트를 CI job lint 에서 check-tools 로 실행해, 도구 테스트를 지정하는 모든 규칙이 건너뛸 수 없는 지점에서 강제되게 한다 | Makefile, tests/tools/workflows.test.mts | `make tool-test-workflows` | [o] |
| S-69 | AGENTS.md 와 AGENTS.ko.md 를 ssr 의 규칙만으로 쓴다: 각 규칙은 Enforced by 줄을 가진 Own 이고, PRINCIPLES.md 의 되풀이와 체크리스트 절차는 제거하며, 두 언어가 같은 check 를 지목한다 | AGENTS.md, AGENTS.ko.md, config/rebuild.json, .gitignore | `make rules-check`, `make docs-check` | [o] |
| S-70 | test 폴더를 test 는 자기 case 를 온전히 적는다는 이유로 중복 검사에서 제외한다: config/duplication.json 이 packages/*/tests/**, packages/*/src/*_tests.rs, tests/** 를 나열한다 | config/duplication.json, config/owner-checks.json | `make duplication-check` | [o] |
| S-71 | make duplication-check 가 보고하는 adapter crate 4개, build, runtime, server 의 source clone 29건을 공유 code 로 재구성해 제거한다. 새 public 이름과 error 타입의 thiserror 사용을 사용자가 승인한 뒤 진행한다 | packages/adapter-*/src/lib.rs, packages/build/src, packages/runtime/src, packages/server/src | `make duplication-check` | [o] |
| S-71-1 | build crate 의 error 를 thiserror 로 derive 하고 임시 경로 제거의 처리를 공유해 Error 와 PublishError 가 code 를 되풀이하지 않게 한다 | Cargo.toml, Cargo.lock, config/dependency-review.json, packages/build/src | `make nextest-build` | [o] |
| S-71-2 | 네 adapter crate 가 공유하는 것(document, assets, render, static shell)을 private crate polyspec-ssr-adapter-support 로 옮기고 error 를 thiserror 로 derive 한다 | packages/adapter-support, packages/adapter-*/src/lib.rs, config/, AGENTS.md | `make nextest-adapter-support` | [o] |
| S-71-3 | adapter 가 생성하는 entry 를 공유 함수 하나로 template 에서 만들어 entry 가 code 를 되풀이하지 않게 한다 | packages/adapter-support, packages/adapter-*/src/lib.rs | `make nextest-adapter-support` | [o] |
| S-71-4 | runtime crate 의 clone 을 제거한다: classic script 의 origin 하나, request 대기의 deadline timer 와 취소·timeout 결과 하나, 병렬 복원 test 의 helper 하나 | packages/runtime/src | `make nextest-runtime` | [o] |
| S-71-5 | development server 의 supervisor 시작을 감시하는 build 와 준비된 build 가 함께 쓰는 함수 하나로 한다 | packages/server/src/development.rs | `make nextest-server` | [o] |
| S-72 | S-60 이후 바뀐 kit 복사본(두 언어의 PRINCIPLES 와 config/php-lint)을 make kit-sync 로 받는다 | PRINCIPLES.md, PRINCIPLES.ko.md, config/php-lint | `make kit-check` | [o] |
| S-73 | kit-gates 의 새 check 인 bypass-check 를 Makefile 의 CHECK_TARGETS 에서 실행한다 | Makefile, config/owner-checks.json | `make kit-check` | [o] |
| S-74 | polyspec-ssr-adapter-support 를 publish = false 로 표시하고 제품이 아닌 공유 구현으로서 config/release.json 의 release 대상 manifest 와 release proof 에서 제거한다 | packages/adapter-support/Cargo.toml, config/release.json | `make kit-check` | [o] |
| S-75 | ownership 검사가 adapter 의 public 이름을 보게 한다: 각 adapter 가 struct 를 선언하고 support crate 의 macro 는 method 만 쓰며, support crate 에 page 와 runtime 을 쓰는 render test 를 둔다 | packages/adapter-*/src/lib.rs, packages/adapter-support, config/test-ownership.json | `make check-ownership` | [o] |
| S-76 | S-72 이후 kit 이 바꾼 PRINCIPLES 복사본을 make kit-sync 로 받는다 | PRINCIPLES.md, PRINCIPLES.ko.md | `make kit-check` | [o] |
| S-77 | CHECK_TARGETS 에서 divergence-check 를 실행하고 config/duplication.json 의 각 reason 을 그 종류(test 또는 harness)로 시작한다 | Makefile, config/duplication.json, config/owner-checks.json | `make divergence-check` | [o] |
| S-78 | 루트의 것만이 아니라 모든 cargo target directory 를 .gitignore 에서 무시한다 | .gitignore | `make check-records` | [o] |
| S-79 | K316 과 K327 부터 K330 으로 kit 이 바꾼 PRINCIPLES 복사본을 make kit-sync 로 받는다 | PRINCIPLES.md, PRINCIPLES.ko.md | `make kit-check` | [o] |
| S-80 | 저장소가 가진 kit 복사본마다 소유 규칙을 둔다 | config/owner-checks.json with clippy.toml in the kit copies rule | `make owner-validate` | [o] |
| S-81 | 스위트가 저장소의 Rust를 위해 kit-gates의 gate인 lint-rust를 실행한다 | Makefile CHECK_TARGETS with lint-rust | `make kit-check` | [o] |
| S-82 | core crate는 오류를 무시하지 않고 필드 오류로 바꾼다 | packages/core/src/lib.rs Page::from_json | `cargo clippy -p polyspec-ssr-core --all-targets --locked -- -D warnings -D unused_must_use -D clippy::let_underscore_must_use -D clippy::let_underscore_untyped -D clippy::map_err_ignore -D clippy::allow_attributes_without_reason` | [o] |
