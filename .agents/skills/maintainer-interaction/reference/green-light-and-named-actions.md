## Green light and named actions

A prompt is either a **named action** (the agent does it, stops at the next boundary) or a **rephrase ask** (the agent renders understanding only, nothing else). The rules below cover each.

### Named actions are their own green light

A concrete named action is its own green light — for that action only. The stop-and-ask boundary is the *next* step, not the work inside the one just named.

Concrete action verbs: `write`, `update`, `refactor`, `add`, `create`, `commit`, `fix`, `rename`, `move`, `delete`, `merge`, `rebase`, `run`, `test`, `build`, `deploy`, `document`, `draft`, `land`.

Vague verbs don't count as named actions — `do something`, `handle that`, `make this happen`, `fix the obvious one`, or any prompt without a concrete verb-target pair. These hit the under-specified-prompt trigger in [reference/communication-quirks.md](reference/communication-quirks.md): stop with a one-line rephrase, freeform, C2 register.

#### Worked shapes

**"Proceed with slice N."** The slice number is the green light for everything inside slice N — file updates, docs, commit, retro, report — and nothing beyond it. The agent finishes slice N, reports, names slice N+1 as the natural next move, and stops.

**"Proceed as proposed" / "as recommended."** This means the *first item* in the proposal just made, not the whole proposal — same per-step shape. A single-item proposal doesn't imply continuation either; it's the green light for that one item only.

Per-step green light is per-step, always — neither phrasing authorizes the steps after the one just named.

#### The exception: pre-scoped multi-step runs

When the maintainer explicitly pre-scopes a run ("do all 10 slices in this order, commit each, report at the end"), each step is still its own green light, but the maintainer has named them all in advance — the agent does the step, reports, names the next, and stops, without needing the green light re-issued each time. Pre-scoping removes the re-ask, not the stop: even "do them all without stopping" is read as "without re-asking," not "without reporting" — the report is part of the slice lifecycle, not a courtesy.

### File writes: a named-action subtype

The maintainer is allowed to be terse about file writes — *"draft the ADR,"* *"update the table,"* *"add a section to AGENTS.md"* — and the agent writes the file without asking permission first. This covers file writes, code changes, and repo operations (`git commit`, branch creation, merge, rebase) whenever the maintainer named the specific action — not when the agent is mid-slice and the operation is normal slice flow.

Two things this doesn't cover:
- **Architectural decisions.** If the named action is itself an architectural call (language choice, module boundary, public API change, cross-cutting concern), the architecture rule governs instead: the named action is the green light for *doing* the decision once the maintainer has made it, not the green light for the maintainer to have made it.
- **The next step** — same scope limit as above, the named action covers only itself.

#### The review shape

1. Write the file.
2. Summarize what landed, in one short line.
3. Name any decisions still sitting with the maintainer, one short line each.
4. Stop.

The chat reply is the summary; the file is the artifact. Don't duplicate the file body in chat, don't preview the content before writing, don't stage in `/tmp` and ask the maintainer to copy it, don't skip the write because the path is read-only or the diff is large, and don't invent alternate paths to dodge a sandbox error — ask for elevation instead.

### Rephrase before further work

When the maintainer asks for a rephrase (or equivalent: "summarize back," "what did you hear," "confirm understanding") before further work, the only acceptable response is the rephrase itself — a single-turn protocol.

Render only what was understood from the maintainer's last message, in the maintainer's own terms:
- what the agent understood the task to be,
- what the agent understood the success criteria to be, if named,
- what the agent is *not* sure about, if anything, in one short line — named as a question, not a request for permission to ask more.

Name any slice, target (file, function, branch, ADR), or constraint (no breaking changes, no new dependencies, register) the maintainer specified. The rephrase is the artifact the maintainer checks — not a status update, not a plan, not a recommendation.

Do not ask clarifying questions, propose options or trade-offs, start slicing/planning/implementing, call tools that move work forward (file edits, task creation, research agents, commit, push, test runs), or duplicate the maintainer's prompt back verbatim instead of rendering the understanding. Then stop and wait.

This holds even if:
- the agent had questions it would have liked to ask — they wait for the maintainer's next turn,
- the rephrase surfaces a problem with the prompt — name it in the "not sure about" line and stop; the maintainer decides whether to refine, redirect, or proceed anyway,
- the agent thinks the rephrase is unnecessary — the maintainer asked, so the agent rephrases.

Scope notes:
- Doesn't apply unless the maintainer asked for a rephrase — the default reply shape (one-line summary, named next move, stop) is unchanged otherwise.
- Doesn't override the named-action rule: if a prompt both names an action and asks for a rephrase, the rephrase is the response, and the named action is held until the maintainer confirms it.
- Doesn't chain with the stop-trigger rules in [reference/communication-quirks.md](reference/communication-quirks.md). If the rephrase turn itself surfaces an ambiguity, the agent stops with a one-line rephrase of *that* ambiguity, not the original task.

### What not to do

- Do not infer continuation — *"proceed with slice 1"* is not *"proceed with slices 1–10."*
- Do not treat a recommendation as implicit consent.
- Do not preemptively call tools that move work forward.
- Do not widen the green light — per-step is per-step.
- Do not skip the stop-and-ask because a run was pre-scoped; pre-scoping names the steps, it doesn't remove the report.