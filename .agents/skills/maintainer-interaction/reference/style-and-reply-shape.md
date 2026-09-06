## Style and reply shape

Brief by default, optimised for scanning, not reading. Maintainers here often have ADHD — a wall of prose buries the win and costs a full turn if they lose the thread and have to ask back.

### Default shape

One of these, in order of preference:

1. **One-line summary + table.** Multiple comparable things — options, files to change, slices done in a batch.
2. **One-line summary + short numbered list.** A few ordered or unordered things to point at.
3. **One-line summary + 1–3 short prose sentences.** Genuinely only one thing to say.
4. **One sentence.** A confirmation or redirect — no table, no list, no header.

Never more than three short prose sentences without a list, table, or header.

Within any shape: lead with the recommendation in the first sentence, don't bury it. Cut filler ("I think," "just," "basically," "really") and hedges that add no information. Give a number only if it matters. Don't restate what the maintainer already said back to them.

**Brief is not silent, vague, or lazy.** State the recommendation. "Maybe we could" isn't one. Brief doesn't mean omitting the file paths, commit message, or next-slice pointer the rest of this skill requires.

### Tables

Default to a table whenever the structure of the information is "rows of comparable things" — options, files to change, constraints, decisions already made, risks. Use prose when the structure is "one thing that needs explanation."

| Option | What it is | Trade-off | When to pick it |
|---|---|---|---|
| A | one-line description | one-line trade-off | one-line trigger |
| B | one-line description | one-line trade-off | one-line trigger |
| C | one-line description | one-line trade-off | one-line trigger |

- One row per option, one column per axis. Cells are short — if a cell needs a paragraph, describe that option in prose after the table instead.
- Always include a recommendation row or a clear marker on one row. A table with no recommendation forces the maintainer to do synthesis work the agent should have done.
- Cite affected files with `path:line` in the cells, not a footnote.

### Do not pad with framing

No "Why this matters," "Why we did it this way," or any preamble explaining *why* the reply is the way it is — the reply is the work; the work is the justification. If a justification is genuinely needed, put it in one short inline sentence in the same paragraph as the recommendation. If it doesn't fit in one inline sentence, it's a different point and belongs in its own list item.

### Wins are first-class

When a slice or step produces real wins, they get their own section near the top of the reply, not buried at the end. The maintainer should be able to read the wins and stop.

> **This landed:** \<one-line summary\>
>
> **Wins**
>
> | What | Why it matters |
> |---|---|
> | \<thing\> | \<one-line reason\> |
>
> **Next:** \<one-line pointer to the next slice, or "done"\>

If a table is too much, at minimum a short numbered list — "wins" must be enumerable, that part isn't optional.

### Trivial actions skip the full shape

The wins-table and numbered-list layout are for non-trivial work: a slice that touched real surface area, an investigation that surfaced a decision, a comparison the maintainer has to read to choose. For trivial actions — `commit everything`, `commit this`, `push`, `run the tests`, `yes do that one thing` — the full shape is grotesque. A one-line confirmation is correct: state what landed in one short sentence, no header, no wins section, no table, no "next" pointer unless the next move is genuinely non-obvious.

The discriminator is action weight, not action type — a commit that only stages already-reviewed work is trivial; a commit that closes a slice with new code in it is not. Both are `git commit`; the reply shape differs.

Examples of trivial actions: `commit everything` / `commit this` / `commit the slice` after the slice content was already reported; `push` once the maintainer has named the branch and local state is clean; `run the tests` / `build it` / `lint` when the run itself is the deliverable, not a finding; a simple `yes` / `proceed` / `go` ending a single sub-step with nothing new to report. The carve-out isn't laziness — the maintainer has already read the work, so the reply is a confirmation, not a re-explanation.

### Never use bare numbers

A bare `slice 7` or `step 2` is a near-guarantee the maintainer has to ask back. Every reference to a numbered thing (slice, step, work item) gets a short inline reminder of what it is.

- Bad: `Slice 7 is done.` Good: `Slice 7 (commit the slice plan) is done.`
- Bad: `Step 2 of 5 next.` Good: `Step 2 of 5 (run the integration test) next.`

When the number came via speech-to-text, double down on the reminder — per the lookup tables in [reference/communication-quirks.md](reference/communication-quirks.md), `slice 3 bravo` should be confirmed inline as `3B (the second sub-slice of slice 3)`; the mapping isn't obvious from the transcript alone.

### Lead with the next action

The slice cycle (code → docs → commit) is the default flow. When a slice lands, name what landed, name the next move, and stop — the maintainer chooses whether to continue.

- Don't write *"Slice 7 done, waiting for you for the next slice"* — that forces the maintainer to ask "is anything left in this run?" Anticipate the obvious follow-up.
- Right shape: *"Slice 7 (matcher routing) landed and committed. The agreed run is complete — next move is your call: a new run with new scope, or a follow-up slice here."*
- When the next slice is obvious and already scoped, name it: *"Next is slice 8 (artifact persistence adapter) — say go and I'll start."*
- Naming the next move is not starting it — stop after naming it.

### Adjust to the situation

The defaults above are defaults. Compress further for trivial confirmations. Expand only when the maintainer has asked for more. A reply that's exactly the right size for the moment beats one that's exactly right for an average moment.