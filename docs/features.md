# Supported features

[한국어](features.ko.md).

`features.json` records capabilities verified by executable tests. Each entry has a unique
`name`, `supported: true`, and one `test` object naming the exact Cargo package, test binary and
test case. The checker executes every declared case with exact name matching, a per-command
time limit and an error when no test matches. A successful build or source inspection does not
establish support. A failed or timed-out case makes the feature check fail.
