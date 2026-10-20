<!-- doc-id: checklist-kit-standard -->
<!-- source-sha256: 9ffb6226c38bfd9271443f25e7e1b0d9ba7ea1b5a851aa5aa156ea19048e7bdd -->
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
| S-71 | make duplication-check 가 보고하는 adapter crate 4개, build, runtime, server 의 source clone 29건을 공유 code 로 재구성해 제거한다. 새 public 이름과 error 타입의 thiserror 사용을 사용자가 승인한 뒤 진행한다 | packages/adapter-*/src/lib.rs, packages/build/src, packages/runtime/src, packages/server/src | `make duplication-check` | [~] |
| S-71-1 | build crate 의 error 를 thiserror 로 derive 하고 임시 경로 제거의 처리를 공유해 Error 와 PublishError 가 code 를 되풀이하지 않게 한다 | Cargo.toml, Cargo.lock, config/dependency-review.json, packages/build/src | `make nextest-build` | [o] |
| S-71-2 | 네 adapter crate 가 공유하는 것(document, assets, render, static shell)을 private crate polyspec-ssr-adapter-support 로 옮기고 error 를 thiserror 로 derive 한다 | packages/adapter-support, packages/adapter-*/src/lib.rs, config/, AGENTS.md | `make nextest-adapter-support` | [o] |
| S-71-3 | adapter 가 생성하는 entry 를 공유 함수 하나로 template 에서 만들어 entry 가 code 를 되풀이하지 않게 한다 | packages/adapter-support, packages/adapter-*/src/lib.rs | `make nextest-adapter-support` | [o] |
