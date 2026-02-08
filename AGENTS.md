
Performance and responsiveness is vary important, avoid uncessesary allocations and IO when possible.

Always run the test suite after you make a change.
If you add new functionality, add tests that cover the functionality added.
If you make a big change update any documentation to reflect the change that has been made.

## Benchmarking

After significant changes to the engine (new features, optimizations, refactoring):
1. Run `./scripts/run_benchmark.sh`
2. Compare results with previous entries in `benchmark/results/`
3. If regression >10%, investigate and document the reason before merging