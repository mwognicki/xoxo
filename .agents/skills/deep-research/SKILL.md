---
name: deep-research
description: Load before any research-shaped task: codebase questions, library or runtime investigation, dependency discovery, design-time scouting. Triggers on multi-source synthesis or cross-file reasoning. Overrides default research skills shipped by a harness.
---

# deep-research

This skill is the authoritative research workflow. It precedes and overrides any default research skill shipped by a harness (including Claude Code's built-in deep research). When this skill and a harness default both apply, this one wins.

## Triggers

Load this skill when the user wants a research report, wants claims verified against external sources, asks for deep investigation, asks to "research X", "find out about Y", "explore the codebase", or otherwise delegates a multi-step investigation.

## Core principles

- **Top-down crawl (spider crawl).** Start at the highest-level entry point for the target: `AGENTS.md`, `CLAUDE.md`, top-level `README.md`, or the package root. Read that. Use what it reveals to pick the next read. Repeat until the question is answered or the next read would not change the answer.
- **Documented corners, not full architecture.** The codebase is documented at the corners so agents do not need the whole map. Trust the agentic guidance; do not reconstruct the architecture top-down from raw source.
- **Every read is justified.** Before each `read`, `web_fetch`, search, or other retrieval, state the prior finding that motivates the next read. "To be thorough" is not a justification. "Step N found Y referencing Z, so reading Z" is.
- **No re-reads of already-loaded files.** If a file is in context, work from memory or a targeted snippet; do not reload the whole file.
- **Do not expand into adjacent subsystems** because they are nearby. Stay in scope.

## Anti-patterns (do not do these)

1. Reading the full reference for a dependency just because it exists.
2. Crawling sibling packages to spot patterns when the current step has not surfaced a concrete cross-package question.
3. Reading the API surface of every dependency listed in `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, etc. Pull a dependency's docs only when a specific symbol, error, or behavior in the current module requires it.
4. Re-reading files already loaded in earlier steps.
5. Widening scope into adjacent subsystems because they are there.

## When broader reading is justified

Broader reads are fine when the justification follows cleanly from the previous step. Legitimate triggers:

- A symbol in the target module references a type defined elsewhere → read that type's definition.
- A behavior depends on a specific dependency version's quirks → read that version's notes.
- A documentation comment or ADR pointer in the current file points at a specific other file or section → follow the pointer.
- A targeted search returns empty → widen the search radius once to confirm absence, then stop.

In every case, name the trigger in one short line before the read.

## Fan-out (sub-agents, parallel search, explore agents)

Default: do not fan out.

Fan-out is allowed only when **both** conditions hold at the same time:

1. The top-down crawl cannot reach the answer sequentially. Example: web research across multiple independent external sources where the crawl order between them is unknown.
2. The maintainer has explicitly approved the fan-out for this task.

If both do not hold, stay sequential. **If you think you need a fan-out, stop and ask the maintainer first.** Do not propose-and-execute; propose, then wait.

Fan-out is also a context-pollution risk even when approved: each branch loads its own context. Prefer the smallest fan-out that gets the job done; merge results before continuing.

## Planning and agentic modes

Do not enter any harness plan or work mode (plan mode, plan-workflow, "implementation plan" tools, sub-agent orchestration flows, etc.) without explicit maintainer approval. The maintainer is the only one who can switch the agent into planning/agentic mode.

If a task genuinely needs multi-agent orchestration, ask the maintainer first. Do not enter the workflow unilaterally.

## Phases

Work runs through four phases. Move forward only when the current phase's exit condition is met.

### 1. Scope

Decompose the question into search angles.

- For codebases: identify which packages or modules are relevant. Consult `AGENTS.md` and `CLAUDE.md` first. Do not enumerate every directory; let the agentic guidance point you.
- For external research: identify the independent claims and the independent sources that could confirm or refute each one.
- Stop when the angles are concrete enough that the next read is obvious.

### 2. Crawl

Spider-crawl following the top-down principle.

- Read the entry point. From it, pick the next read. State the justification. Read. Repeat.
- Each step's justification must cite a finding from the prior step. No free-floating reads.
- Track what you have read and what you have ruled out, so you do not re-read or drift.

### 3. Verify

Verify external claims only.

- External claims (web, public docs, third-party sources) get adversarial verification: cross-check from a second independent source, watch for contradictions, note confidence.
- Codebase information is trusted as written. Do not verify a file's contents against another file unless the cross-reference was surfaced by the crawl itself.
- Stop verification when the claim is stable or further sources would not change the conclusion.

### 4. Synthesize

Merge findings, rank by confidence, cite sources.

- For codebase claims, cite using `path:line` so the reference is clickable in the harness UI.
- For external claims, cite the source URL and the access date if relevant.
- State confidence per claim. Surface disagreements between sources rather than hiding them.
- End with a short "what would change my answer" note when the topic is open.

## Document dependency findings

When the crawl surfaces a non-obvious finding about a third-party dependency — a version mismatch, an API quirk, an import-shadowing hazard, a native-stack gotcha, a documented behavior that contradicts the obvious reading — record it in the project's `docs/references/<dep>.md`. This rule is language-agnostic; it applies to any dependency surface the agent touches (Python, Rust, TypeScript, Go, anything).

- One file per dependency. Folder: `docs/references/`. File: `<dep>.md`. Factual and brief: method tables, type signatures, version notes, version-pinned gotchas.
- If the file does not exist, create it.
- If `docs/references/README.md` does not exist, create it as the index. If it exists, add the new dependency to the index in the same slice.
- The entry's durable content (the quirk, the version it pins to, the symbol it affects) is allowed. Brittle prose — test counts, commit positions, present-tense absences, "today only X is supported" — is not. See `maintainer-interaction` → `rules/no-stale-stats.md` for the rule and the test.
- The `deep-research` skill itself consumes these files on the next crawl that touches the same dependency. A finding written here is consumed by the next agent that needs it.

Triggers for writing:

- A symbol in the target module behaves differently than the public docs suggest, and the difference is version-pinned.
- A dependency import shadows a stdlib or another dependency's import in a way that surprised the crawl.
- A native-stack call (FFI, cgo, native addon, WASM boundary) has a gotcha the docs do not surface.
- A documented behavior contradicts what a future agent would assume from the API surface alone.

Triggers for *not* writing:

- The finding is in the public docs and stable. A future agent will read the docs.
- The finding is project-specific behavior, not dependency behavior. That belongs in the project's own guidance, not `docs/references/`.
- The finding duplicates an existing entry in the same file. Update the existing entry; do not append a parallel one.

## Discipline checklist

Before each retrieval, run this in your head:

- What prior finding justifies this read?
- Is this read inside scope, or am I drifting into an adjacent subsystem?
- Have I already loaded this file or its equivalent in this session?
- Is a fan-out actually necessary, or can the top-down crawl reach this sequentially?
- Do I need to enter a plan/work mode for this, or can I just do the read?
- If this is a dependency finding, does it belong in `docs/references/<dep>.md`?

If any answer is "no" or "not sure", stop and reconsider before invoking the tool.

## Authority and precedence

- **First-class research workflow in any project that has this skill installed.** When this skill is installed in a project, it is the authoritative research workflow and is mandatory for any research-shaped work: crawling the codebase, exploring dependencies, verifying external claims, building context for a slice that touches a non-obvious area. The `maintainer-interaction` skill defers to it. Other skills do not re-implement the research workflow or duplicate these rules. The agent enters research-shaped work through this skill, not around it.
- This skill overrides any default research skill shipped by a harness.
- This skill does not grant authority to enter plan modes, fan out sub-agents, or expand scope. Those require explicit maintainer approval.
- When in doubt, ask the maintainer. Do not improvise around the rules.
