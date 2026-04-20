---
name: rust-best-practices
description: Rust coding conventions, architecture rules, and style requirements for this repository. Load when writing, reviewing, or refactoring Rust code.
---

# Rust Best Practices

## Architecture

- **Thin entrypoint.** `main.rs` wires components only — no business logic, validation, or orchestration. Business logic belongs in library modules, never in entrypoints.
- **Plain helpers over container-bound services.** Small stateless reusable helpers that don't need dependency injection should stay plain functions rather than being wrapped in structs or service containers.
- **Module autonomy.** Modules depend on each other only through stable public interfaces (traits or public types). No reaching into internals. No circular dependencies.
- **Stable external interfaces guard against refactoring creep.** Public API surfaces (`pub` traits, structs, functions) should be treated as contracts. Changing them forces churn across all callers. When in doubt, extend rather than modify — add new methods, newtypes, or wrapper traits instead of altering existing signatures.
- **Cross-module calls must go through stable interfaces.** Never `use` another module's internal types or helpers directly. Depend only on what the owning module exposes as `pub`. This keeps refactoring local — internals can change freely as long as the public contract holds.
- **Decoupling.** Keep specific implementations (protocol adapters, transports) separate from core logic. Design interfaces so adding new implementations is straightforward.
- **Directory structure enforces domain boundaries.** Group by logical domain (e.g., `config/`, `persistence/`, `cli/`), not by technical utility. Module ownership must be unambiguous — every piece of behavior has one clear home.
- **Shared logic → library crate.** If multiple binaries emerge, extract shared types and logic into a lib crate.
- **DRY over explicitness.** When logic is genuinely duplicated, abstract it. Repeating code "to be explicit" is a code smell — extract a shared function, type, or trait instead.
- **Low hidden complexity.** Prefer small implementation surfaces, shallow call stacks, and straightforward module ownership. If a reader needs to jump through five files to understand a flow, the architecture needs simplifying.
- **Generalize when it's natural.** If functionality could be generalized, shared, or open-sourced, that option is worth pursuing. Prefer a well-factored shared utility over a narrow one-off.

## Code style

- **File size limit: 400 lines.** Split approaching files.
- **Import order:** `std` → external crates → internal modules, separated by blank lines.
- **Formatting:** `rustfmt` defaults. No stylistic debates.
- **Warnings as errors:** `#![deny(warnings)]` in the crate root.
- **No suppressed lints** (`#[allow(...)]`) without an inline comment explaining why.

## Error handling

- **Library code:** `thiserror` for structured, typed errors.
- **Application/binary code:** `anyhow` where concrete error types aren't critical for callers.
- **No `unwrap()` or `expect()` in production paths** unless accompanied by `// SAFETY:` or `// INVARIANT:` comment explaining why panic is impossible.

## Interface design

- **Idiomatic signatures** over short-term simplicity.
- **Simplicity over premature optimization.** No hidden magic — prefer explicit, observable flows.
- **Trait-based abstraction** for behavior contracts (enables swapping and mocking).
- **Shared types** live in an appropriate shared module, not duplicated or exposed through internals.
- **Export the smallest stable surface consumers actually need.** Hide everything that isn't required externally. Less exposed surface means less API churn and freer internal refactoring.
- **Design APIs as if they will be consumed by an unknown external caller.** Even internal modules should have clean, self-documenting interfaces — the discipline pays off when boundaries shift or code gets extracted.
- **Internal implementation changes must not force consumer changes** as long as the public interface is preserved. If a refactor breaks callers, the public surface was either wrong or the refactor overreached.

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
- **Unit tests** live in the module they test. If test logic exceeds ~30 lines, move to a sibling `module_tests.rs`.
- **Integration tests** go in `tests/` at the crate root.
- **External resources:** tests handle their own setup/teardown (e.g., temp directories).

## Refactoring discipline

- **Strict scope.** Prefer minimal, composable changes that align with the existing architecture instead of broad structural rewrites. No opportunistic cleanup or reorganization.
- **Correctness first.** Fidelity to instructions over architectural neatness. Do not expand scope without explicit approval.

## Complexity control

- **Unnamed or poorly understood complexity is a critical risk.** If you can't explain it plainly, don't introduce it.
- **Smallest implementation surface that satisfies the requirement.** When one query, one branch, or one step can replace several, prefer the simpler shape.
- **Every additional query, branch, check, or moving part must be justified** by a real requirement — not speculative precision.
- **Every abstraction needs a clear purpose, clear location, and clear name.** If it lacks any of these, it shouldn't exist.
- **No hidden complexity.** No implicit behavior, unclear abstractions, or scattered logic without clear ownership.
