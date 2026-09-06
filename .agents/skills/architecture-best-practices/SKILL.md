---
name: architecture-best-practices
description: Load when making architecture-level decisions across modules, packages, or services. Cross-cutting; language skills may override individual rules.
---

# Architecture Best Practices

Direction for shaping a system's structure. Project-agnostic. Language-neutral.

This skill is the **default heading**. Specific language skills (TypeScript, Python, Rust, Go, etc.) restate these principles in language-idiomatic form and may tighten, loosen, or override individual rules when the language genuinely cannot follow them. Per-language restatements are authoritative for that language; this skill is the common baseline.

## When To Use

Load this skill for any change that touches:

- module, package, crate, or service boundaries
- public API surface and its stability contract
- cross-cutting concerns (auth, error handling, observability, deployment topology)
- data model and storage shape
- dependency direction between layers
- workspace layout (lib area vs bin area, monorepo package boundaries)

Process, scope, and slice discipline are owned by `maintainer-interaction`. For research-shaped work, follow `deep-research` first. This skill assumes the architecture and the current state of the codebase are already known.

## Override Posture

When this skill and a language-specific skill pull in different directions, the language skill wins **for language-shape** rules (idiom, syntax, naming shape, type-system mechanics). For architecture-shape rules (boundaries, layering, dependency direction, public API stability), this skill wins unless the language skill explicitly overrides.

Example: SOLID's interface-segregation principle is not fully expressible in Go. The Go skill owns that override. Module boundaries, layering, and dependency direction remain as stated here.

A language-specific override must be explicit in that skill's body. "Implicit" overrides do not count.

## Core Goals

Optimize for:

- stable boundaries
- clear contracts
- low hidden complexity
- small implementation surfaces
- understandable module ownership
- incremental delivery — never break the build. Ship the smallest change that keeps the existing behavior intact and layer improvements on top.
- strict reuse of existing code — before introducing a new helper, type, or module, search the codebase for an existing equivalent. Duplication is a tax; reuse is the default. Only create new when nothing fits.

## Module Boundaries

How pieces of the system are sliced into modules, packages, crates, or services, and how they talk to each other.

### State Lives With Its Behavior

Behavior that operates on the same data structure belongs with that data structure. The unit of state-and-behavior is the type, class, module, or struct — not a collection of loose module-level functions that share state across call sites.

Stateless helpers stay plain. Helpers that share state do not — they belong as methods on the owning type, or as a single owner module that exposes them through a stable interface.

### Stable Public API

Anything exported from a module — capitalized identifier, `pub` item, re-exported from a top-level `__init__.py`, listed in a package's public surface — is a contract. Changing it forces churn across every importer.

The discipline:

- **Do not rename exported symbols casually.** Pick a name that survives the next three refactors; once it sticks, keep it.
- **Do not reorder parameters or positional arguments on public callables.** Callers depend on position. Add an optional parameter at the end; do not shuffle existing ones.
- **Prefer additive changes.** New helpers, new optional parameters, new exports — these are backward-compatible. Breaking changes require explicit approval.
- **Internal refactors keep the public call shape intact.** The refactor is internal if and only if no caller has to change a single line.

### Stable API as a forcing function on extraction

The "stable public API" rule above is not just a guideline for symbols that already exist — it is also a forcing function on the *moment* of extraction. When a module is carved out into its own reusable unit — a library, a package, a sibling, or any other self-contained surface — the public surface that lands on day one is the surface every future caller will depend on. Renaming or reshaping it later costs churn across every consumer; getting the shape right on day one is nearly free.

The principle has two halves:

1. **Structural extraction when growth is foreseeable.** When a concern is going to grow into a separate logical subtree (auth, config, persistence, telemetry, anything with more than one rule or source), extract it into its own reusable unit *now*, even at a handful of lines. The cost is one manifest entry, one entrypoint file, and a single re-export line in the parent. The benefit is that every future addition lands inside a stable seam instead of accreting into the calling module or into an overstuffed neighbor. The day-one shape is the day-N shape.

2. **Operation-oriented, not data-oriented, public API.** Callers describe what they need done; the owning module owns how it gets done. A caller that needs "is this token valid?" should call `validate(token)` and never see the token. A caller that needs "give me the configured token" should call `resolve_token()` and never see the source. The lookup, the source format, and the rules can all change freely; the contract stays put.

Concrete discriminator when extracting:

- **Expose the operation callers need.** `validate(x)`, `resolve()`, `connect()` — verbs, not nouns.
- **Hide the data behind the operation.** Constants, lookup tables, and config keys stay internal. If a caller can name the internal data, the seam is wrong.
- **The day-one shape is the day-N shape.** Pick the API as if five consumers already exist. If the shape would not survive five consumers, do not extract with a different shape now.

The cost of getting extraction right on day one is near zero. The cost of getting it wrong and reshaping later is the kind of opportunistic refactor across every caller that produces spaghetti code. Stable API discipline *is* the discipline that prevents spaghetti at the module boundary.

### Cross-Module Calls Go Through Stable Interfaces

Consumers depend on the owning module's public surface only. They never reach into internals via deep imports, private types, or undocumented helpers. The contract is the public surface; the internals are the implementation.

Concretely:

- A package / module exposes a public API. Everything else is private to the package and may change without notice.
- A consumer imports the public surface. The consumer never imports a file marked internal, private, or under a directory conventionally read as internal.
- When the public surface needs to evolve, the evolution happens additively (see "Stable Public API"). The internal implementation can move freely.

This is what makes refactoring local: a well-shaped boundary means the internals of module A can be rewritten in any way, and module B does not care as long as the contract holds.

### One Concept Per File Or Module

A file or module owns one cohesive concern. The signal that the principle is being violated:

- A flat directory of 8–10+ files of the same kind (e.g. ten `utils.ts` files) is a missed-cohesion signal — look for natural groupings.
- A file mixes unrelated concerns (e.g. `auth.ts` that also handles logging, HTTP retries, and UI rendering).
- A module's public surface requires the reader to know about three unrelated subsystems to understand it.

Splitting for splitting's sake is also a smell. A small file that genuinely owns one cohesive concern is fine even if it is short. The principle is cohesion, not raw line count.

### Public / Internal Type Split

Structural artifacts (interfaces, type aliases, DTOs, schemas, contracts) used by more than one executable module belong in a dedicated location separate from the executable code that consumes them. The split has three rules:

- **Structural artifacts live in a dedicated directory / sibling module.** The exact name varies by language (`src/types/`, `types.ts`, `types.rs`, `<pkg>/types/`). The principle is the same: one place to look for contracts.
- **Executable modules import from there.** The dependency direction is one-way: executables depend on contracts, contracts do not depend on executables.
- **The split is binding once a package has more than one executable module.** Before that, inline is fine; the duplication cost has not yet justified the indirection.

Why: when contracts and executables share a file, a change to either ripples into the other. A consumer who imports the contract also pulls the implementation; a consumer who imports the implementation also pulls the contract. The two audiences are different; the file should serve one.

## Layering And Dependency Direction

### Framework-Agnostic Domain, Framework Surface In A Sibling

Domain logic (data structures, algorithms, I/O, persistence, generic contracts) and the layer that exposes it to a framework (agent runtime, RPC stack, UI toolkit, web framework) are different concerns. They must not live in the same module or library root.

When a layer depends on a framework:

- The framework surface lives in a sibling module, crate, or package.
- The domain layer stays framework-agnostic so it can be recomposed by other front-ends (tests, other agents, CLIs, services).
- The split is mandatory at the module or crate boundary even when the framework dependency would technically compile inside the domain module. Compile-time enforcement is the point — a runtime or convention-based split leaks.

The concrete shape of the split (sibling crate, sub-module, feature flag) is the project's call. The rule is that the dependency direction is one-way and enforced at compile time.

Why this matters: a framework-agnostic domain can be tested without spinning up the framework; it can be exposed through multiple front-ends (CLI, web, agent); it can be replaced wholesale when the framework changes. A domain that is tangled into the framework has none of those properties.

### Protocol Adapters Separate From Domain

Keep transport, persistence, and protocol adapters separate from core domain logic. The domain code does not know about the transport it is exposed through.

Shape:

- Domain code defines an interface for what it needs (e.g. `load(id) → Entity`, `save(entity) → Result`).
- An adapter implements that interface against a specific protocol (HTTP, gRPC, Postgres, Redis, a model provider).
- The domain code calls the interface; the adapter is interchangeable.

The win: adding a new transport (say, swapping Postgres for SQLite in tests, or adding gRPC alongside REST) is one new adapter, not a domain-code rewrite. Swapping an adapter does not ripple into the consumer.

### Decoupling

Keep specific implementations behind narrow interfaces. The shape:

- **The consumer declares the interface it needs.** The interface lives where it is consumed, not where it is implemented.
- **The producer implements a concrete type.** The producer does not need to know about the interface; it just exposes its public surface, and the consumer's interface is satisfied by structural or nominal match.
- **Interfaces are small.** One to three methods is the typical range. Larger interfaces are usually a sign that the seam is wrong, or that several seams were collapsed into one.

Avoid contextual binding hacks: swapping behavior implicitly between environments (test vs prod, dev vs staging) through context variables, runtime registries, or environment-conditional imports is fragile. Make the environment explicit at the boundary; keep the rest of the code straightforward.

### Workspace Layout

Library modules / crates / packages are reusable, framework-agnostic, and importable by anything else in the workspace. Binary modules / crates / entrypoints wire the libraries and exit; they do not own domain logic.

The default layout for a multi-package workspace is a split:

- **Library area** — reusable packages. Importable, framework-agnostic, no business logic tied to a specific entrypoint.
- **Binary area** — entrypoints that wire libraries together and run. `main`, `cmd/`, application packages.

The split makes the dependency direction obvious from the path alone and stops binaries from growing domain code by accident.

For TypeScript monorepos, the split is enforced by the monorepo toolchain (Turborepo, Nx, etc.):

- Library packages live under `packages/` (or the workspace's library area).
- Application packages live under `apps/` (or the workspace's binary area).
- The monorepo toolchain enforces the dependency direction: `apps/*` depend on `packages/*`, never the reverse.

A flat layout — every package at the workspace root with no separation between library and binary areas — is acceptable only when the project explicitly opts in. Treat the flat layout as opt-in, not as the easy default.

### Shared Logic Moves To A Shared Package

If multiple binaries, services, or applications emerge, extract shared types and logic into a dedicated library package. The bar for extraction is intent, not duplication:

- **"Will more than one consumer want this?"** is the right question. If yes, promote.
- **"Has more than one consumer already grabbed a copy?"** is the wrong question. By that point, you have already paid the duplication tax; the only remaining question is whether to keep paying it or to consolidate.

When logic is genuinely duplicated, abstract it. Repeating code "to be explicit" is a code smell — extract a shared function, type, or trait instead. The goal is real explicitness (clear naming, clear ownership) over false explicitness (duplication that does not aid reading).

## Interface Discipline

The shape of the contracts that bind the system together. Boundaries are interfaces, not concrete implementations.

### Prefer Interfaces And Contracts At Boundaries

A boundary is an interface, schema, or abstract contract — not a concrete implementation. Behavior is the dependency; the concrete type that implements it is interchangeable.

The discipline:

- **At a module boundary, expose an interface** (in the language's idiom: a TypeScript `interface`, a Rust `trait`, a Go interface, a Python `Protocol` or abstract base class). The implementation is a separate concern.
- **Behavior is the dependency.** When a consumer needs "something that loads an entity by id," it depends on the loading behavior — not on the specific database or HTTP client that happens to provide it.
- **Implementations are interchangeable.** Swapping one implementation for another is a localized change; it does not ripple into consumers.

### SOLID-Shaped Design

Keep SOLID principles in mind when shaping modules, contracts, and responsibilities. The principles are language-neutral in direction, even when they are not fully expressible in every language:

- **Single Responsibility** — a module has one reason to change. A change in one concern should not force a change in unrelated concerns.
- **Open/Closed** — extend behavior through new code, not by modifying existing code that already works.
- **Liskov Substitution** — subtypes are substitutable for their base types without the consumer noticing.
- **Interface Segregation** — consumers depend on narrow interfaces, not fat ones. A consumer should not have to import a 20-method interface to use two of them.
- **Dependency Inversion** — high-level modules depend on abstractions, not on concrete implementations.

Some of these principles are not fully expressible in every language. Interface Segregation, for instance, is structurally limited in Go (where interfaces are satisfied implicitly and a small interface per consumer is the idiom, but the broader principle of narrow seams applies). The language-specific skill owns the override; the direction remains.

### Avoid Fat Services

Split orchestration, validation, transformation, and persistence into focused units:

- **Orchestration** wires the steps together; it does not own the steps.
- **Validation** raises at the boundary; it does not flow through core logic.
- **Transformation** is a pure function; it does not perform I/O.
- **Persistence** owns the storage shape; it does not own the domain logic.

When a service owns all four, it is a fat service. Split it.

### Small Stateless Reusable Helpers Stay Plain

Helpers that do not need dependency injection should stay plain helpers — not wrapped in structs, service containers, or IoC-managed components. The wrapper adds ceremony without changing what the helper does.

The discriminator: does the helper need state, configuration, or injected dependencies to do its job? If yes, it is a method on a type or a service. If no, it is a plain helper.

### Avoid Contextual Binding Hacks

Do not swap behavior implicitly between environments (test vs prod, dev vs staging) through context variables, runtime registries, or environment-conditional imports.

The problem:

- A function's behavior depends on a global that the caller cannot see.
- A test that works locally fails in CI because the environment is different.
- A bug fix in one code path is silently bypassed in another because the binding swapped.

The fix:

- Make the environment explicit at the boundary. Configuration is loaded once and passed to the components that need it.
- Avoid runtime registries that decide which implementation to use based on a flag the caller did not set.
- When an environment-specific implementation is genuinely needed, declare it as an explicit alternative and let the caller pick — not a hidden lookup.

## File And Module Size

### The 400-Line Soft Cap

Unless a language-specific skill states otherwise, the soft cap is **400 lines per file**.

Why 400: it is large enough to hold a meaningful concern without forcing premature splits, and small enough to read in one screen at a comfortable font size. Combined with the principle of "one concept per file," the line cap is a forcing function for cohesion — when a file is clearly going to keep growing, split it before it does.

### How To Apply The Cap

- **For new files, the cap is binding.** Start under it.
- **For edits to existing files still below the cap, keep them below it** if reasonably possible.
- **For edits that would push a file past the cap, split into sibling files or move into a sub-package / sub-module.** The split is the obvious move; do not let the file grow casually.
- **A landing at 402 or 405 lines is acceptable** when the overage is minor or mostly comments, blank lines, or other low-density content. Treat the limit as a reasonable cap, not a rigid hard stop.
- **For files that already exceed the cap before the current change:** do not treat that alone as a reason to refactor. Do not proactively split or restructure them unless the maintainer explicitly asks. Keep the requested change focused unless the maintainer wants cleanup as part of the task.

### Splitting For Size

When a file is expected to accumulate significant logic — because its domain is naturally broad, or because the maintainer has asked for an explicit split — prefer splitting over letting the file grow:

- **Sibling files in the same directory or package.** The package re-exports the public surface so existing import paths stay stable.
- **Sub-package / sub-module.** When the split crosses a meaningful boundary (a different concern, a different audience), promote to a sub-package rather than just a sibling file.
- **Keep the entrypoint / barrel lean.** The barrel or `__init__` re-exports the public surface; it does not define new logic. All implementation lives in the sibling files.

### What This Is Not

- Not a magic number. The cap is a tool for keeping diffs and reviews small, not a rule carved in stone. Apply the spirit (cohesion, small surface, one concept per file) when the literal line count would force a worse shape.
- Not a refactoring trigger. Files that already exceed the cap before the current change are not the agent's problem unless the maintainer explicitly asks.
- Not a substitute for cohesion. A file with 200 lines of unrelated concerns is worse than a file with 450 lines of one cohesive concern. The principle is cohesion first; the cap is a forcing function, not a goal.

## Input Validation, I/O Seams, And Settings

The trust boundary, the I/O boundary, and the configuration boundary. Each one is a place where discipline pays for itself.

### Validate Inputs At The Boundary

One explicit validation layer at the trust boundary. The boundary is wherever untyped or external data enters the system:

- HTTP handlers and middleware
- RPC entrypoints
- Tool arguments and tool schemas
- Config loaders
- Struct / class / dataclass constructors
- Artifact loads (JSON, YAML, files from disk)

After the boundary, core logic stays straightforward. Validation raises at the boundary, not deep inside core logic where the failure mode is unclear.

The discipline:

- **Wrong runtime object types** raise a type error (`TypeError`, typed error, `Result::Err`).
- **Invalid values or impossible state** raise a value error (`ValueError`, typed error, `Result::Err`).
- **Error messages are concrete enough** for callers (and tests) to act on. Vague messages are a smell — they are written for the developer who already understands the bug, not the next person to debug it.
- **Persistence and tool failures must be visible.** Never silently produce a run that looks resumable when its state was not saved. A failure that is not surfaced is a failure that will be hit again, harder to diagnose.

### Injectable I/O Seams

Network, filesystem, model, and external-service I/O go behind interfaces or injected callables. Tests avoid live I/O when a seam can be faked.

The shape:

- A function or method that performs I/O takes its dependencies (the network client, the filesystem, the model provider) as constructor parameters or as optional injected callables.
- The I/O seam is narrow: one to three methods on a small interface is the typical range.
- Tests inject a fake (in-memory, deterministic, no network) at the call boundary. The fake implements the same interface as the real dependency; the code under test does not know the difference.

What this is not:

- A live network or live-model dependency in tests. The seam exists so this does not happen.
- A heavy mocking framework that requires generated mocks and runtime reflection. A small struct that implements the seam interface is usually clearer and just as effective.
- An excuse to wrap every helper in a service container. Stateless helpers that do not need I/O stay plain.

### No Hidden Side Effects

Helpers that should stay pure stay pure. The discipline:

- A function whose name suggests a pure transformation (e.g. `format_date`, `parse_config`, `validate_input`) does not write to a database, send a network request, or modify a global registry.
- Module-level functions with implicit dependencies on global state, environment variables, or filesystem reads are a smell. Make the dependency explicit at the call site or inject it.
- When a side effect is necessary, name it (e.g. `save_to_db`, `send_request`, `publish_event`). The reader should be able to tell, from the call site, which functions are pure and which are not.

### Configuration Behind A Settings Surface

Avoid scattered direct access to environment variables (`os.getenv`, `process.env`, `std::env::var`, ad-hoc env reads) in feature code.

The shape:

- Runtime configuration lives behind a single settings surface (a config struct, a `Settings` class, a pydantic-settings model — whatever the project already uses).
- Environment variables are the **input**. The loaded config struct is what feature code **sees**.
- The settings surface is loaded once at startup and passed explicitly to anything that needs it.
- Feature modules depend on the config struct (or on a narrow interface into it), not on raw environment lookups.

Why this matters:

- A scattered `os.getenv` access pattern means the configuration surface is implicit. Every feature module is its own source of truth for what config it reads.
- A single settings surface makes the configuration auditable: one place to look for what the system reads from the environment.
- Tests can construct a config struct with known values. With scattered env reads, tests have to mutate the process environment, which is fragile and order-dependent.

### Tests Live Near The Code They Cover

- Unit tests sit next to or inside the unit they cover (colocation).
- Integration tests live in a top-level test directory (`tests/`, `__tests__`, `src/tests/` — whatever the project's convention is).
- Tests are deterministic — no sleep-based timing. Use fake timers, channels, barriers, or injected clocks.
- Mock at the call boundary (the function the production code calls) only when the test asserts call shape or argument marshaling, not the service's actual behavior. For service-behavior tests, prefer the most realistic seam that still avoids the network: in-memory backends, ephemeral test servers, fake services that implement the same interface.

## Complexity Control And Naming

The two disciplines that keep an architecture honest over time: keeping complexity small, and letting names carry the load.

### Unnamed Or Poorly Understood Complexity Is A Critical Risk

If you cannot explain a piece of logic plainly, you do not yet understand it well enough to introduce it. The instinct to add a layer of abstraction "to be safe" almost always produces complexity that does not pay for itself.

The test: take any piece of logic in the codebase and explain it in two short sentences. If you cannot, simplify it until you can.

### Prefer The Smallest Implementation Surface

Prefer the smallest implementation surface that correctly satisfies the requirement. When one query, one branch, or one step can replace several, take the simpler shape.

The default:

- One way to do a thing, not three.
- One level of indirection, not two.
- One location for a behavior, not a fallback chain.

### Every Additional Moving Part Must Be Justified

Every additional query, branch, check, or moving part must trace to a real requirement — not speculative precision, not "we might need this later". Every abstraction needs:

- a clear purpose (what problem it solves)
- a clear location (where the concern lives)
- a clear name (what to call it at the call site)

If it lacks any of these, it should not exist.

### Hidden Complexity Is Forbidden

Avoid:

- implicit behavior (functions that mutate state without saying so)
- unclear abstractions (interfaces whose contract is not documented or self-evident)
- scattered logic (the same behavior implemented in three places)
- ownership gaps (state read or written from places that have no clear owner)

If a construct cannot be explained plainly, do not introduce it.

### Names Are Documentation

Names signal role, audience, or purpose at the path. A reader who opens a directory tree should be able to tell, from the file names alone, what each piece of code does.

The discipline:

- **Pick a naming convention and apply it consistently.** Whether it's kebab-case, snake_case, or PascalCase, the convention should be uniform across the same kind of file.
- **Use suffixes to signal role.** When a file's role is not obvious from its name, append a role suffix: `.service.ts`, `.schema.ts`, `.repository.ts`, `.utils.ts`, `.types.ts`, `.config.ts`. The suffix is a documentation aid, not a hard rule — append it whenever the role would already appear in the name anyway.
- **Avoid junk-drawer names.** `helpers.ts`, `utils.ts` (without scope), `misc.ts`, `stuff.rs` are signs that the file has accumulated unrelated concerns. Split it.
- **Multi-word names beat single-word ambiguous names.** `user-profile.ts` over `profile.ts` (which collides with too many other things).

### No Shadowing Of Dependencies, Stdlib, Or Siblings

A local unit (module, package, file, directory) whose name matches a stdlib unit, a third-party dependency, or another top-level unit the same file also imports creates a name-resolution hazard. The next agent that reads the file will not know which `redis` or `http` is meant.

Three shapes to watch:

- **Local unit vs stdlib unit.** A directory or module whose name collides with a stdlib package (`io`, `net`, `http`, `os`, `time`, `context`, `sync`, `errors`, `fmt`, `path`, `crypto`, `collections`, `asyncio`, `pathlib`, `typing`, `uuid`, `json`) creates an import-shadowing hazard.
- **Local unit vs third-party dependency.** A directory or module whose name matches a published dependency (`redis`, `tokio`, `serde`, `reqwest`, `pydantic`, `mongoengine`) shadows it for any consumer in the same scope.
- **Local unit vs sibling top-level unit.** Two top-level units in the same project cannot share a name; the build rejects this, often with a misleading error.

How to fix:

- **Rename the local unit.** Append a disambiguating suffix (`_local`, `_internal`, `_adapter`, `_wrapper`, `_helpers`, `_backend`, `_client`, `_utils`, `_types`, or a project-specific noun). The user's project convention is one of those suffixes; any consistent suffix is fine as long as the local name no longer collides.
- **Delete the local unit when it is a thin wrapper or a placeholder.** A single-function re-export that exists only to be re-imported is almost always unnecessary; inline it at the call site and remove the file.
- **Do not paper over the shadowing with import aliases at every call site.** Repeated `as` aliases across the codebase are a smell; rename the local unit instead.

The fix is the rename or the delete, not the import alias.

## What Belongs In Repository-Local Instructions Instead

Project-specific material stays out of this skill, for example:

- framework-specific exceptions
- repository-specific import conventions
- project-specific time libraries or env managers
- local file-size limits that differ from the 400-line baseline
- local naming deviations
- branch-specific or workflow-specific rules
- toolchain pins (language-specific; lives in the language skill and in the project's own toolchain manifest)

Those belong in repository-local instructions such as `AGENTS.md`.

## Practical Defaults

When adding or changing code here:

1. Start from the smallest backward-compatible change.
2. Keep interfaces typed and explicit; prefer extending public types over modifying their signatures.
3. Extract pure helpers before adding branching to public entrypoints.
4. Add focused test coverage for the changed branch.
5. Preserve public exports unless asked not to.
6. Use the project's standard layout and toolchain as defined in the language skill and in the project's own `AGENTS.md`.

## Avoid

- breaking re-exported public interfaces without approval
- ad-hoc environment access outside the configuration surface
- hidden side effects in helpers that should stay pure
- silently swallowed errors and persistence failures
- introducing new framework-level patterns when existing stdlib or current project patterns already fit
- growing a single file past the 400-line soft cap when a split is the obvious move
- module-level functions that share state and would be cleaner as methods on the owning type
- local units whose name shadows a dependency, stdlib unit, or sibling top-level unit

## Refactoring Discipline

### Strict Scope

Prefer minimal, composable changes that align with the existing architecture instead of broad structural rewrites. No opportunistic cleanup or reorganization. Correctness first: fidelity to the requested slice over architectural neatness.

### Generalize When It Is Natural

If functionality could be generalized, shared, or open-sourced, that option is worth pursuing. Prefer a well-factored shared utility over a narrow one-off. Surface the suggestion to the maintainer; do not expand the slice.
