<!-- doc-id: checklist-kit-standard -->
# Kit standard checklist

| ID | Task | Deliverables | Verification | State |
|---|---|---|---|---|
| S-58 | Adopt the plan folders of kit: the checklist moves as tables into docs/plans/execution/ and docs/plans/kit-standard/, config/checklist.json declares the ID pattern, and the changelog moves to CHANGELOG.md and CHANGELOG.ko.md of the root | docs/plans/execution/, docs/plans/kit-standard/, config/checklist.json, CHANGELOG.md, CHANGELOG.ko.md | `make docs-check` | [o] |
| S-59 | Set the PHP floor of config/php-lint to the platform PHP 8.4 (kit-sync copy of config/php-lint/composer.json and composer.lock) | config/php-lint/composer.json, config/php-lint/composer.lock | `make toolchain-check` | [o] |
| S-60 | Take the copies of kit 0.0.54 with make kit-sync: the lint configurations, the layout of kit, PRINCIPLES, the release and gates workflows, the bootstrap and .claude/settings.json | the copies named by kit.copies of @polyspec/kit | `make kit-check` | [o] |
| S-61 | Let the workflow test accept the gates workflow of kit as a copy of kit, beside release.yml | tests/tools/workflows.test.mts | `make tool-test-workflows` | [o] |
