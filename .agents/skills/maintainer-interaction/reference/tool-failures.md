## Tool failures and garbled output

Two failure modes that look similar from the outside (something the agent trusted turned out wrong) but have different causes and recoveries. One is a real signal about the world; the other is a signal about the transport. Mixing them up costs the agent either a missed update or a hallucinated one.

### Tool failures: the tool is the evidence

**The tool is right until proven otherwise.** Do not double down on your model. The cost of a stale model is high — every subsequent read, recommendation, and slice lands on a false picture, and a tool failure is the cheapest signal you'll get that the picture is wrong.

1. **Note the contradiction in one line.** Not a paragraph, not an apology: "Tool returned X, I expected Y."
2. **Update the model.** Whatever the tool returned is now the working picture — re-derive downstream beliefs from it, not the old one. Do not silently absorb the contradiction; quietly re-deriving from a wrong model is worse than loudly updating it.
3. **Re-check related state.** A change in one place often means a change in adjacent places. A quick, targeted re-read with a stated justification is fine; an open-ended "let me re-look at the codebase" is not.
4. **Proceed.** Do not loop on the failure, and do not re-run the same tool hoping for a different answer — if the result is the same, the result is the same.

If the change can't be explained by your own recent actions, treat it as an external actor (another tool, a hook, a CI job, a human edit, a worktree operation): note it briefly, one line in the slice plan, then proceed.

**Escalate to the maintainer when:**
- the tool returns structured errors that block the slice and the slice plan doesn't cover them,
- the contradiction implies a previous slice was mis-implemented and the rollback path is non-obvious,
- the cost of the update (touching N files, retracting a recommendation) exceeds what one tool error can justify.

### Garbled output: never invent the missing parts

Custom inference routing and token-optimization plugins sit between the model and the tools — sometimes several at once. As a result, tool output or rendered prompts can look off: truncated mid-sentence, reordered (a list that no longer matches its headings, a diff with hunks out of order), partially rewritten (reads like a paraphrase of itself), or elided (`...` or empty ranges). This is an artifact of the routing layer, not corruption of the underlying data — the file on disk is fine, and the user didn't actually say the garbled thing.

**Never invent or guess the missing content.** The garbage is not a hint — filling it in from context is hallucination, and the next slice gets built on it. This holds however small the gap looks: don't paraphrase a garbled error into a "best guess" and act on it, don't fill elided ranges with plausible-looking content, and don't pretend the output was clean — the maintainer will read the actual tool output later, and a confident agent that quietly guessed wrong costs more time than a paused one that asks.

**If the output is not essential** (cosmetic): note it in one line ("note: tool output looked reordered") and proceed with what's usable. Don't block the slice on cosmetics.

**If the output is essential** (a test result, compiler error, a file to edit, a security-sensitive value, a contract to follow):

1. Retry the same tool call once — routing is usually idempotent across retries, and a second pass often returns clean.
2. If still garbled, try a narrower read (smaller range, different format flag) to bypass the artifact.
3. If still garbled after a small number of attempts, **stop and consult the maintainer.** Do not press on with assumptions or propose a slice built on a guessed-at error message.

### How the two differ

A tool failure says *the world is different from what I thought*; garbled output says *the transport is different from what I thought* — the discriminator is the cause, not the symptom. If the underlying file or process changed, it's a tool failure. If the file on disk is fine and only the rendered output is mangled, it's garbled output.

Confusing the two produces two distinct failure shapes: treating garbled output as a tool failure updates the model to garbled text and acts on it — hallucination in the slice. Treating a tool failure as garbled output means retrying hoping for clean transport, getting the same result, and asking the maintainer unnecessarily — a lost slice.

When the discriminator is unclear, treat it as a tool failure (update the model, do not retry) — under-updating is recoverable, over-updating is not.