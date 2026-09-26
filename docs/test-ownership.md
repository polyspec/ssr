[Korean](test-ownership.ko.md)

# Test ownership

`tools/test-ownership.json` declares shared page and build behavior. Each behavior names one
integration test in its owning crate and one actual use test in every crate that directly depends
on that owner, including development dependencies. A declaration gives the crate, integration
test target and exact test function. A file name alone does not identify an executable case.

`tools/test_ownership.py` compares each consumer set with the workspace manifests, checks that
each named function is a nonempty test in its own crate, and executes every named test with
`cargo nextest`. Missing, misplaced, empty, failed, ignored and timed-out cases fail. The checker
reports each case and its result; `make check` runs it after package installation and before
the workspace test suite. This check establishes test execution and location. The named tests
assert the behavior's input and output in their own crates.
