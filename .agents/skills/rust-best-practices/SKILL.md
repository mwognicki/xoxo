---
name: rust-best-practices
description: Load when writing, reviewing, refactoring, analyzing, or researching Rust code. Restates architecture rules in Rust-idiomatic form; may override individual rules.
---

# Rust Best Practices

Project-agnostic style, architecture, and conventions for Rust work. Applies whenever Rust is being written, reviewed, or refactored in a project where this skill is installed. Pair with the project's own `AGENTS.md` / `CLAUDE.md` for the local crate layout, dependency policy, and project-specific overrides.

## When To Use

Load this skill for any Rust change that touches:

- `main.rs` / `lib.rs` entrypoints, public API surface, re-exports
- module structure, domain boundaries, crate splits
- runtime configuration, error handling, persistence, transport, or tooling seams
- tests under `src/tests/`, `tests/`, or inline `#[cfg(test)]` modules
- the dependency manifest (`Cargo.toml`, `Cargo.lock`)

Process, scope, and slice discipline are owned by `maintainer-interaction`. When this skill and `maintainer-interaction` pull in different directions (this skill favors generalizing or extending code; `maintainer-interaction` favors the smallest sensible step), `maintainer-interaction`'s "smallest sensible step" wins. The generalization becomes a suggestion to the maintainer, not a license to expand the slice. For research-shaped work, follow `deep-research` first; this skill assumes the architecture and the current state of the codebase are already known.

## Architecture

### Thin entrypoint

`main.rs` and `lib.rs` wire components and define the public API surface only. Business logic, validation, helpers, and orchestration live in submodules (`src/foo.rs`). Entrypoint files exist to declare the module tree, re-export the public interface, and hold the crate-level doc comment — nothing else. Even private helper functions and test scaffolding belong in submodules, not inlined in the entrypoint.

### Plain helpers over container-bound services

Small stateless reusable helpers that do not need dependency injection should stay plain functions rather than being wrapped in structs or service containers.

### Module autonomy

Modules depend on each other only through stable public interfaces (traits or public types). No reaching into internals. No circular dependencies.

### Stable external interfaces guard against refactoring creep

Public API surfaces (`pub` traits, structs, functions) are contracts. Changing them forces churn across all callers. When in doubt, extend rather than modify — add new methods, newtypes, or wrapper traits instead of altering existing signatures.

### Cross-module calls go through stable interfaces

Never `use` another module's internal types or helpers directly. Depend only on what the owning module exposes as `pub`. This keeps refactoring local — internals can change freely as long as the public contract holds.

### Decoupling

Keep specific implementations (protocol adapters, transports) separate from core logic. Design interfaces so adding new implementations is straightforward.

### Directory structure enforces domain boundaries

Group by logical domain (e.g. `config/`, `persistence/`, `cli/`), not by technical utility. Module ownership is unambiguous — every piece of behavior has one clear home.

### Workspace layout: split into lib area and bin area

The default for a multi-crate workspace is a split layout: one area for library crates, one area for binary crates. Library crates are reusable, framework-agnostic, and importable by anything else in the workspace. Binary crates are entry points — `main.rs` thin-wires the libraries and exits; they do not own domain logic. Keeping the two kinds in separate areas makes the dependency direction obvious from the path alone and stops binaries from growing domain code by accident.

A flat layout — every crate at the workspace root, with no separation between lib and bin areas — is acceptable only when the maintainer has explicitly said so for the project. Treat the flat layout as opt-in, not as the easy default.

### Shared logic → library crate

If multiple binaries emerge, extract shared types and logic into a lib crate.

### DRY over explicitness

When logic is genuinely duplicated, abstract it. Repeating code "to be explicit" is a code smell — extract a shared function, type, or trait instead.

### Low hidden complexity

Prefer small implementation surfaces, shallow call stacks, and straightforward module ownership. If a reader needs to jump through five files to understand a flow, the architecture needs simplifying.

### Generalize when it is natural

If functionality could be generalized, shared, or open-sourced, that option is worth pursuing. Prefer a well-factored shared utility over a narrow one-off. Surface the suggestion to the maintainer; do not expand the slice.

### Framework-agnostic domain, framework surface in a sibling

Domain logic (data structures, algorithms, I/O, persistence, generic traits) and the layer that exposes it to an agentic, UI, or framework surface are different concerns and must not live in the same module or library root. When a layer depends on a framework — agent runtime, RPC stack, GUI toolkit — the framework surface lives in a sibling module or crate, and the domain layer stays framework-agnostic so it can be recomposed by other front-ends (tests, other agents, CLIs, services). The split is mandatory at the module or crate boundary even when the framework dependency would technically compile inside the domain crate — compile-time enforcement is the point. The concrete shape of the split (a single sibling crate, a sub-module, a feature flag) is the project maintainer's call; the rule is that the dependency direction is one-way and enforced at compile time.

## Code style

### Edition and toolchain

The default Rust edition is **2024**. New crates declare `edition = "2024"` in `Cargo.toml`; existing crates upgrade on the maintainer's call, not as a side effect of any single change.

The minimum supported toolchain is **rustc 1.97** at the time of writing. This floor tracks the host's current stable: bump it in this skill and in the project's `rust-toolchain.toml` together. Do not pin to a version below the floor in `Cargo.toml` or in CI; the floor exists to keep the project's feature surface predictable.

The concrete pin lives in `rust-toolchain.toml` at the repo root, not in prose scattered across `AGENTS.md` files. The skill and `AGENTS.md` reference the pin; the pin is the source of truth.

### File size limit: 400 lines

The 400-line cap applies to `*.rs` files. Other source files (`*.js`, `*.ts`, `*.html`, CSS, etc.) are not bound by the same hard cap, but should still be split when it is reasonable to do so — when a clear submodule boundary exists, when the file mixes unrelated concerns, or when navigation becomes painful. The cap is a tool for keeping diffs and reviews small, not a magic number; apply the same spirit to every file type.

### Import order

`std` → external crates → internal modules, separated by blank lines.

### Formatting

`rustfmt` defaults. No stylistic debates.

### Warnings as errors

`#![deny(warnings)]` in the crate root.

### Clippy policy

**Clippy** is the preferred linter, alongside `rustfmt` and `deny(warnings)`. The recommended strict invocation is `cargo clippy --all-targets --all-features -- -D warnings`; the project's exact clippy invocation (and any stricter-than-default lints) is the project's call, recorded in the project's own `AGENTS.md`.

Clippy complaints are not priority fixes. The discriminator is whether the code compiles and behaves correctly without the fix:

- If a clippy lint blocks compilation, fix it as part of the slice that introduced it. That is a real bug, not a style nit.
- If a clippy lint is stylistic, suggestive, or "you could do this differently", **flag it in the slice retro and stop.** Do not spend tokens and turns chasing the lint as if it were a compile error. The maintainer sees the flagged finding, decides whether it is worth a follow-up slice, and says so. Most clippy findings of this kind are not.
- If a clippy lint suggests a refactor that is also an architectural decision (extracting a trait, splitting a module, renaming a public type, etc.), surface it as an option in the standard A/B table shape, recommend briefly, and stop. Architectural decisions belong to the maintainer; clippy is not the maintainer.

The point is not that clippy is unimportant. It is that the agent's job on a slice is to land the slice correctly, and clippy noise is a signal *to* the maintainer, not a work queue *for* the agent. Fixing clippy issues the maintainer has not prioritized burns context that should go to the actual slice.

### No suppressed lints

`#[allow(...)]` is forbidden without an inline comment explaining why.

## Error handling

- **Library code:** `thiserror` for structured, typed errors.
- **Application or binary code:** `anyhow` where concrete error types are not critical for callers.
- **No `unwrap()` or `expect()` in production paths** unless accompanied by `// SAFETY:` or `// INVARIANT:` comment explaining why panic is impossible.

## Interface design

- **Idiomatic signatures** over short-term simplicity.
- **Simplicity over premature optimization.** No hidden magic — prefer explicit, observable flows.
- **Trait-based abstraction** for behavior contracts (enables swapping and mocking).
- **Shared types** live in an appropriate shared module, not duplicated or exposed through internals.
- **Export the smallest stable surface consumers actually need.** Hide everything that is not required externally. Less exposed surface means less API churn and freer internal refactoring.
- **Design APIs as if they will be consumed by an unknown external caller.** Even internal modules should have clean, self-documenting interfaces — the discipline pays off when boundaries shift or code gets extracted.
- **Internal implementation changes must not force consumer changes** as long as the public interface is preserved. If a refactor breaks callers, the public surface was either wrong or the refactor overreached.

## Crate naming conflicts

### Module files must not shadow dependencies, stdlib re-exports, or siblings

A local module or `mod` declaration whose name matches a Cargo dependency, a `pub use` re-export from a sibling crate, or another top-level module the same file also imports creates a `use`-resolution shadowing hazard. Rust's name resolution can be ambiguous when two units share a name in the same scope, and even when it resolves correctly, the next agent that reads the file does not know which `redis` or `serde` is meant.

The rule is the same in shape across all three language best-practices skills: do not name a local unit identically to any dependency, sibling top-level unit, or stdlib re-export you also import. The fix is to rename the local one, not to qualify `use` statements at every call site.

Three shapes to watch:

- **Local module vs. Cargo dependency.** A file like `src/redis.rs` with `mod redis;` shadows the `redis` crate in any `use` inside `src/`. The same applies to any other top-level dependency (`tokio`, `serde`, `reqwest`, `bullmq`, `mongodb`).
- **Local module vs. sibling top-level module.** Two top-level modules in the same crate cannot share a name; Rust rejects this at compile time, but the failure message is often misleading. If you genuinely need two modules with the same logical name, one must be in a sub-module (`a/foo.rs` and `b/foo.rs` is fine; `foo.rs` and `bar/foo.rs` of the same name at the crate root is not).
- **Workspace member name vs. external crate name.** A `[workspace] members` entry whose crate name matches a published crate on crates.io can pull the wrong package in some toolchains. Use a distinct crate name in `Cargo.toml` (e.g. `acme-redis` rather than `redis`).

Bad name pairs to avoid by default. The local module is the left column; the thing it shadows is the right column. Illustrative, not exhaustive.

| Local name (do not use) | Shadowed unit | Common source |
|---|---|---|
| `src/redis.rs` | `redis` crate | dependency |
| `src/bullmq.rs` | `bullmq` crate | dependency |
| `src/tokio.rs` | `tokio` crate | dependency |
| `src/serde.rs` | `serde` crate | dependency |
| `src/reqwest.rs` | `reqwest` crate | dependency |
| `src/mongodb.rs` | `mongodb` crate | dependency |
| `src/uuid.rs` | `uuid` crate | dependency |
| Workspace member `redis` | published `redis` crate | workspace / dependency |

How to fix:

- **Rename the local module.** Append a disambiguating suffix: `_backend`, `_client`, `_adapter`, `_utils`, `_types`, `_local`. The user's project convention is `_backend` / `_utils` / `_types`; any consistent suffix is fine as long as the local name no longer collides.
- **Delete the local module when it is a thin wrapper or a placeholder.** A `mod` that exists only to re-export a dependency, or a single-function wrapper, is almost always unnecessary; remove the file and use the dependency directly.
- **Do not paper over the shadowing with `as` aliases at every call site.** `use redis::Client as RedisClient;` repeated across the codebase is a smell; rename the local module instead.

The fix is the rename or the delete, not the import alias.

## Configuration

- **Environment variables** for runtime config.
- **Explicit and simple** — not hot-reloadable unless a specific requirement demands it.

## Documentation

Every `pub` item gets a `///` doc comment with:

1. **One-sentence description** of purpose.
2. `# Errors` — when it returns `Result::Err`.
3. `# Panics` — conditions causing a panic (if any).
4. `# Examples` — for non-trivial public APIs.

## Testing

- **No `sleep`-based timing.** Use channels, barriers, or mocks for determinism.
- **Unit tests** live in the module they test. If test logic exceeds roughly 30 lines, move it into `src/tests/{tested_file}.rs` and wire it back to the tested module with `#[cfg(test)] #[path = "tests/{tested_file}.rs"] mod tests;`, so the file stays a submodule of the code it tests (private access via `super::*`) while all test files sit together under `src/tests/`.
- **Integration tests** go in `tests/` at the crate root.
- **External resources:** tests handle their own setup and teardown (temp directories, ephemeral ports, scoped environment mutation).

## Refactoring discipline

- **Strict scope.** Prefer minimal, composable changes that align with the existing architecture instead of broad structural rewrites. No opportunistic cleanup or reorganization.
- **Correctness first.** Fidelity to instructions over architectural neatness. Do not expand scope without explicit approval.

## Complexity control

- **Unnamed or poorly understood complexity is a critical risk.** If you cannot explain it plainly, do not introduce it.
- **Smallest implementation surface that satisfies the requirement.** When one query, one branch, or one step can replace several, prefer the simpler shape.
- **Every additional query, branch, check, or moving part must be justified** by a real requirement — not speculative precision.
- **Every abstraction needs a clear purpose, clear location, and clear name.** If it lacks any of these, it should not exist.
- **No hidden complexity.** No implicit behavior, unclear abstractions, or scattered logic without clear ownership.

## Practical Defaults

When adding or changing Rust code here:

1. Start from the smallest backward-compatible change.
2. Keep interfaces typed and explicit; prefer extending public types over modifying their signatures.
3. Extract pure helpers before adding branching to public entrypoints.
4. Add focused unit and integration test coverage for the changed branch.
5. Preserve public exports unless asked not to.
6. Use the project's standard toolchain as defined in the 'Edition and toolchain' rule above; do not invent a different Rust version or rely on ambient `cargo` from a path that bypasses the project pin.

## Avoid

- breaking re-exported public interfaces without approval
- broad `#[allow(...)]` usage without an inline justification
- ad-hoc environment access outside the configuration surface
- hidden side effects in helpers that should stay pure
- silently swallowing `Result::Err` flows
- introducing new framework-level patterns when existing stdlib or current project patterns already fit
- growing a single `*.rs` file past the 400-line cap when a split is the obvious move
- module-level free functions that share state and would be cleaner as methods or as a thin wrapper struct
