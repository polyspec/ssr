<!-- doc-id: checklist-kit-standard -->
<!-- source-sha256: fe76350745dfb90db857884fcfe93c3f0e8ed459852ae8f4b38d83492c14a743 -->
# Kit 표준 체크리스트

| ID | Task | Deliverables | Verification | State |
|---|---|---|---|---|
| S-58 | kit 의 계획 폴더를 채택한다: 체크리스트를 표로 docs/plans/execution/ 과 docs/plans/kit-standard/ 로 옮기고, config/checklist.json 에 ID 패턴을 선언하며, 변경 기록을 루트의 CHANGELOG.md 와 CHANGELOG.ko.md 로 옮긴다 | docs/plans/execution/, docs/plans/kit-standard/, config/checklist.json, CHANGELOG.md, CHANGELOG.ko.md | `make docs-check` | [o] |
| S-59 | config/php-lint 의 PHP 하한을 플랫폼 PHP 8.4 로 맞춘다 (kit-sync 가 쓰는 config/php-lint/composer.json 과 composer.lock) | config/php-lint/composer.json, config/php-lint/composer.lock | `make toolchain-check` | [o] |
| S-60 | make kit-sync 로 kit 0.0.54 의 복사본을 받는다: lint 설정, kit layout, PRINCIPLES, release 와 gates workflow, bootstrap, .claude/settings.json | the copies named by kit.copies of @polyspec/kit | `make kit-check` | [o] |
| S-61 | workflow test 가 release.yml 과 함께 kit 의 gates workflow 를 kit 복사본으로 받아들이게 한다 | tests/tools/workflows.test.mts | `make tool-test-workflows` | [o] |
| S-62 | config/owner-checks.json 이 kit 0.0.54 의 복사본과 문서 경로를 소유하게 한다: documents-check 대신 docs-check, gates.yml 과 .claude/settings.json 은 kit-check 가 소유한다 | config/owner-checks.json | `make owner-validate` | [o] |
| S-63 | Makefile 의 CHECK_TARGETS 가 kit-gates 의 모든 gate 를 실행하게 한다: documents-check 대신 docs-check 를 쓰고 rules-check, rebuild-check, language-check, test-markers-check, duplication-check, parity-declarations-check, parity-check, commits-check, link-state-check, lint-ts 를 추가한다 | Makefile | `make kit-check`, `make tool-test-workflows` | [o] |
| S-64 | make hooks 가 쓰는 kit 0.0.54 의 pre-commit hook 을 pre-push 와 함께 추적한다 | .githooks/ | `make hooks-check` | [o] |
| S-65 | scripts/ 의 도구 소스 7개를 config/rebuild.json 에서 분류한다 (S1): ecosystem, 또는 지키는 규칙을 적은 policy | config/rebuild.json, config/owner-checks.json | `make rebuild-check` | [o] |
