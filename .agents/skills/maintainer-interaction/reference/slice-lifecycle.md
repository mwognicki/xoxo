## Core expectation: iterative minimum-effort steps

Large, one-shot implementations are discouraged. Break every non-trivial piece of work into **slices**. A slice is the smallest useful increment that:

- is **non-destructive** — does not break the current state of the code or docs,
- is **non-breaking** — leaves the system working at every commit,
- is **traceable** — lands in one commit (or one tightly scoped change),
- is **scoped** — small enough to be reviewed, redirected, or reverted,
- leads naturally toward the final form, but is not the final form.

When asked "what's next," propose the smallest meaningful slice that improves the system or reduces uncertainty. Prefer the next reasonable step over the grand final form.

## Slice lifecycle (four steps, every slice)

1. **Implement.** Write the code, doc, or config the slice calls for. Update the slice plan where it's kept (project plan, issue body, scratch doc). Append a short note recording what landed, what deviated, and any `ponytail` simplifications (deliberate shortcuts with a stated upgrade path) that were retired. The slice lands with the repo at least as operational as it was before — see "Repo operational state" below.
2. **Update agentic guidance.** If the slice touches more than one package, crate, module, or doc, update every `AGENTS.md` / `CLAUDE.md` / skill that owns part of the slice so the next agent that crawls in finds the new state. Strip brittle prose (test counts, branch position, present-tense absences) on the way through. Single-file slices skip this step. Test for brittle prose: read the line out of context, six months from now, with no other commits to compare against — if it wouldn't still be true, don't write it.
3. **Commit.** Commit to the current branch unless the maintainer said otherwise ("don't commit yet," "leave it staged," "I'll commit"). Commit message follows the conventional format the repo already uses; don't invent a new style. See **Harness delivery** for cross-harness sandbox quirks affecting this step (Codex permission re-prompt, git identity on retry).
4. **Report.** Tell the maintainer what landed and what the next slice is, or that the plan is done. The report is the green-light boundary: name the next move, stop. See "Green light and named actions" and "Style and reply shape" → "Lead with the next action."

## Linting timing

Defer linting and formatting to after the last substantial slice in a multi-slice run — not after each individual slice. Running the linter every slice wastes effort, since later slices can resolve issues earlier ones would have flagged. Run once, fix once.

### What counts as "linting"

Language-agnostic; tool depends on the language:

| Language | Linter | Formatter |
|---|---|---|
| Rust | `clippy` | `rustfmt` |
| TypeScript / JavaScript | `eslint` | `biome` / `prettier` |
| Python | `ruff` | `ruff format` |

### When to run per-slice

Single-slice work, or the final slice of a multi-slice run: lint and format as part of "implement + verify," before the commit. Mid-run slices: skip linting — build and test are enough to catch breakage.

If the maintainer asks for a lint check mid-run, do it — their explicit instruction overrides this default.

## Reporting style

Keep the report short. The maintainer is reading dozens of these.

- One sentence for what landed. Form: "This landed: `<one clause>`." Not a paragraph, not a diff recap, no praise, no apology.
- One sentence for the next slice. Form: "Next slice: `<number>` — `<one clause>`." Or, if finished: "Next slice: none — implementation is done."

Do not restate the slice plan or enumerate every file changed — the diff and commit message already do that.

## Slice enumeration

- Top-level slices: Arabic numerals — `1`, `2`, `3`, `4`, ...
- Sub-slices (when a slice is itself broken into smaller parts): capital Latin letters — `A`, `B`, `C`, ...
- A sub-slice is named by parent slice + letter: the second sub-slice of the third slice is **`3B`**, the first is **`3A`**. Never `slice 3 sub 2`, never `3.2`, never `3-b`.

## Repo operational state

The repo must never sit in a temporary, buggy, or non-operational state without the maintainer's explicit confirmation and consent — at every boundary: between slices, between a commit and its follow-up, between a merged branch and the next step.

### The rule

Every slice lands with the repo at least as operational as it was before the slice. If a slice breaks something that previously worked, the corrective work is part of that slice — the slice does not land until the repo is at least as operational as it started.

### What "operational" means

The repo can be built, tested, run, deployed, and observed at the level it could before the slice. Concrete signals:

- CI / CD pipelines run.
- Publish paths work.
- Build scripts exit zero.
- Type checks pass.
- Migrations are paired with their reverse.
- Feature flags are not left dangling.
- No file is referenced by a tracked file but missing from the working tree.
- No silent breakage: a path that worked before still works, a flag that was honored before is still honored, a config key that was read before is still read.

### What does not count as "operational" (i.e., these are fine)

- `ponytail`-marked simplifications: deliberate shortcuts with a stated upgrade path. The retro records the ponytail; the slice lands.
- Documented TODOs.
- Missing tests for low-priority interactive surfaces (see the maintainer's interactive-surface rule where it applies).
- Intentional scope cuts the maintainer agreed to.

These are "incomplete by design, with the maintainer's consent" — not "broken." The retro names them.

### When corrective work doesn't fit in the current slice

If folding the fix into the current slice would grow it past reasonable scope, land the working slice, name the broken state in the retro, and make the corrective slice the *very next one worked on* — never let a broken state span more than one unlanded slice.

This is a planning rule, not just a recovery rule: the slice plan should avoid creating the broken window in the first place, not merely recover from it after the fact. Naming a regression in the retro is required but is the minimum acceptable — the slice order itself should have avoided creating it. If a slice does go sideways after landing anyway, the same rule applies: the corrective slice is the next thing worked on, but it shouldn't have been necessary to plan for it.

### When folding is not feasible

If the corrective work needs an external dependency the working slice doesn't have (a library version, a CI secret, a teammate's review, a deploy window), stop and ask the maintainer explicitly for consent before landing the working slice. Do not assume the maintainer accepts a broken state because it's "recoverable" — their consent is the only thing that lets a broken state land.

Three consent shapes:

- *"Yes, land the working slice, the broken state is fine for now"* — the agent lands, names the broken state in the retro, and the corrective slice is next.
- *"No, do not land; fold the corrective work in"* — the agent does not land; the working slice and the corrective work land together.
- *"Yes, but the corrective slice is N, not N+1"* — the agent lands, names the broken state, and the corrective slice goes at the maintainer's chosen position.

The agent does not pick. The maintainer picks.

## Opportunistic guidance / skill touch-ups between slices

Slices land back-to-back; between a commit and its follow-up is
a natural window for small, surgical fixes to agentic guidance
(`AGENTS.md` files, `AGENTS.md` slice-status lines, skill files
under `.agents/skills/`). These touch-ups are part of the
opportunistic-cleanup category that `git-policies` and the
project rules call out — they do not require their own slice.

The window is narrow:

- A touch-up lands **after** a slice's commit and **before**
  the next slice starts, as its own commit on the same branch.
  Slices and skill fixes are separate commits even when they
  ship in the same turn.
- The touch-up must not break the repo's operational-state
  promise above. A skill file edit that takes the repo into a
  broken state is not opportunistic — it is a slice that was
  never scoped.
- Identity rules for these commits are the same as for slice
  commits: pass nothing to `git commit`, fall through to host
  config, stop on a missing identity. See
  `git-policies/SKILL.md` §"Identity" — the same sequence
  applies to every commit, slice or otherwise.

### Slice plan check

When proposing a slice plan, check it for this property before presenting it: if the plan leaves a broken window between slice N and slice N+1, either fold the corrective work into N, fold it into N+1, or stop and ask the maintainer.