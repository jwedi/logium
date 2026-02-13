# Logium Development Guidelines

## After Every Change

Run the full CI pipeline checks before considering work complete:

### Rust (from project root)

1. `cargo fmt --check` — formatting
2. `cargo clippy --workspace -- -D warnings` — linting
3. `cargo test --workspace` — all unit and integration tests

### Frontend (from `ui/`)

1. `cd ui && npm run format:check` — Prettier formatting
2. `cd ui && npm run check` — svelte-check + TypeScript
3. `cd ui && npm test` — Vitest test suite

### Benchmarks

After significant changes to the Rust engine (new features, optimizations, refactoring):

1. Run `./scripts/run_benchmark.sh`
2. Compare results with previous entries in `benchmark/results/`
3. If regression >10%, investigate and document the reason

## TODOS.md Tracking

When working on a task from `TODOS.md`, update its entry to **Done** with a short summary of what was changed when the task is complete.

## General

- Performance is important — avoid unnecessary allocations and IO
- Add tests for new functionality
- Update documentation for significant changes
