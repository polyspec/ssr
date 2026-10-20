<!-- doc-id: checklist-kit-standard -->
<!-- source-sha256: 57ea7bb6e26675d818e9c05eec1418ca69e8b1081186ab754aa7b0c8562a65aa -->
# Kit 표준 체크리스트

| ID | Task | Deliverables | Verification | State |
|---|---|---|---|---|
| S-58 | kit 의 계획 폴더를 채택한다: 체크리스트를 표로 docs/plans/execution/ 과 docs/plans/kit-standard/ 로 옮기고, config/checklist.json 에 ID 패턴을 선언하며, 변경 기록을 루트의 CHANGELOG.md 와 CHANGELOG.ko.md 로 옮긴다 | docs/plans/execution/, docs/plans/kit-standard/, config/checklist.json, CHANGELOG.md, CHANGELOG.ko.md | `make docs-check` | [o] |
| S-59 | config/php-lint 의 PHP 하한을 플랫폼 PHP 8.4 로 맞춘다 (kit-sync 가 쓰는 config/php-lint/composer.json 과 composer.lock) | config/php-lint/composer.json, config/php-lint/composer.lock | `make toolchain-check` | [o] |
| S-60 | make kit-sync 로 kit 0.0.54 의 복사본을 받는다: lint 설정, kit layout, PRINCIPLES, release 와 gates workflow, bootstrap, .claude/settings.json | the copies named by kit.copies of @polyspec/kit | `make kit-check` | [o] |
| S-61 | workflow test 가 release.yml 과 함께 kit 의 gates workflow 를 kit 복사본으로 받아들이게 한다 | tests/tools/workflows.test.mts | `make tool-test-workflows` | [o] |
