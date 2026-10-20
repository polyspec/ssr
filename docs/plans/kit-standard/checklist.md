<!-- doc-id: checklist-kit-standard -->
# Kit standard checklist

| ID | Task | Deliverables | Verification | State |
|---|---|---|---|---|
| S-58 | Adopt the plan folders of kit: the checklist moves as tables into docs/plans/execution/ and docs/plans/kit-standard/, config/checklist.json declares the ID pattern, and the changelog moves to CHANGELOG.md and CHANGELOG.ko.md of the root | docs/plans/execution/, docs/plans/kit-standard/, config/checklist.json, CHANGELOG.md, CHANGELOG.ko.md | `make docs-check` | [o] |
| S-59 | Set the PHP floor of config/php-lint to the platform PHP 8.4 (kit-sync copy of config/php-lint/composer.json and composer.lock) | config/php-lint/composer.json, config/php-lint/composer.lock | `make toolchain-check` | [o] |
