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

## Working Through TODO Items

When asked to complete multiple items from `TODOS.md`, process them **one at a time** in this cycle:

1. **Plan** — Enter plan mode. Research the codebase, read all affected files, and write a detailed implementation plan.
2. **Review** — Present the plan for user approval. Do not proceed until approved.
3. **Implement** — Follow the approved plan. Make the code changes and add tests.
4. **Verify** — Run the full CI pipeline (see "After Every Change" above). Fix any failures.
5. **Commit** — Create a git commit with a clear message summarizing the change.
6. **Update TODOS.md** — Mark the item as Done with a short summary.
7. **Next** — Move to the next item and repeat from step 1.

Do not batch or parallelize items — they often touch shared files or have implicit dependencies.

## General

- Performance is important — avoid unnecessary allocations and IO
- Add tests for new functionality
- Update documentation for significant changes
