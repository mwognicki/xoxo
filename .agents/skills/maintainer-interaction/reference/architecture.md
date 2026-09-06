## Choices belong to the maintainer

**The agent proposes; the maintainer decides.** That's the whole rule. Architecture is a stronger instance of this same rule — covered below — but the mechanism is the same everywhere: present, recommend briefly, stop.

### What "presenting options" looks like

A short table (the default), or a short bulleted list: one row per option, one column per axis, one line per cell. Mark the recommendation in one short line — no further justification. The maintainer doesn't need a paragraph to know why the agent prefers A; they need the preference named so they can override it cleanly.

| Option | What it is | Trade-off | When to pick it |
|---|---|---|---|
| A | one-line description | one-line trade-off | one-line trigger |
| B | one-line description | one-line trade-off | one-line trigger |

> Recommended: A, because <one short clause>.

(Match the situation — don't copy this shape verbatim.)

### What "stopping" looks like

After the table, the agent stops. No "shall I proceed with A?" No "I'll start on A while you decide." No "sounds good? then I'll…" The next turn is the maintainer's — they pick, ask a follow-up, or redirect. Silence is not consent; if the maintainer doesn't respond, the agent doesn't start.

This holds even when the agent has a strong preference and the "obvious" choice looks small and safe. The cost of a second turn is small. The cost of acting unilaterally is real — it removes the maintainer's agency at exactly the moment they were about to exercise it.

### Scope of this rule

- It fires when the agent is *presenting* options — not every decision requires that pause. Many decisions are already inside a named action, and the agent makes them as part of doing that action.
- It doesn't override the green-light rule: if the maintainer has named a specific option ("do A"), the agent does A. This rule governs what happens *after* options are presented, not whether they're presented at all.

## Architecture: the stronger case

Anything that would materially lock in the shape of the system needs the maintainer's sign-off *before any code lands* — a recommendation alone isn't enough, unlike the general case above.

### What counts as architectural

Non-exhaustive:

- Choice of language, runtime, framework, or major library.
- Module, package, or service boundaries.
- Data model and storage shape.
- Public API surface and its stability contract.
- Cross-cutting concerns: auth, error handling, observability, deployment topology.
- Build, test, and release strategy.

Heuristic: if a change would be hard to reverse cheaply, it's architectural.

### How to act when in doubt

1. Read the root-level `AGENTS.md` and `CLAUDE.md` of the project.
2. Read any `docs/adr/`, `docs/architecture/`, or equivalent, and the current code in the affected area.
3. Propose the design as a draft: name the decision, list the options, give a recommendation, name the trade-offs, link the affected files with `path:line`.
4. Stop. Wait for the maintainer.

An agent may draft an ADR or contract and send it for review — but does not self-approve it.

### Dictated decisions are signed off

The maintainer dictates decisions directly in some prompts. Two flows, two ADR statuses:

- **Dictated with certainty** ("we're switching to NestJS", "MongoDB stays secondary") — the dictation *is* the sign-off. The ADR lands directly as **Accepted**, dated the day of dictation. No Proposed interim state, no separate approval round-trip.
- **Options flow** ("what are our options?" → discussion → "put it down as an ADR") — the ADR lands as **Proposed** until the maintainer accepts it.

When the register of a prompt is unclear, land the ADR as **Proposed** and name that choice in one line of the slice report.

### Overrides

The rule is overridden only by **explicit, unambiguous** text in the root-level agentic guidance of the project. "Implicit" overrides, "the obvious read," "this is consistent with how we do things" — none of these count. If the override isn't on the page, the rule holds. When an override does exist, name the file and line in the recommendation so the maintainer can see the basis at a glance.

## What not to do (applies throughout)

- Do not pick one and start implementing it without the maintainer's green light.
- Do not treat a recommendation as implicit consent.
- Do not preemptively call tools that move work forward ("I'll fix option 1 now…").
- Do not merge "here are two options" with "I'll do the first one" in the same message — that collapses a decision into a fait accompli.
- Do not offer a multi-choice question form. The maintainer types the original sentence back; the agent doesn't hand them a guess list.
- Do not frame a recommendation as a fait accompli in the slice report.
- Do not discover an existing ADR and quietly route around it.