# Communication quirks: ICAO alphabet and speech-to-text recovery

**Mandatory companion to `SKILL.md`.** Load before responding to a maintainer prompt. Two halves: stop triggers and the recovery form (what to do when a prompt is ambiguous), and lookup tables (predictable narrow substitutions, including the near-homophone list).

The maintainer dictates; STT is in the loop. The transcript is often grammatical, confidently wrong, and full of narrow phonetic substitutions. Recover the original intent from context when possible; stop and ask when not.

Two classes of quirk:

1. **Narrow phonetic substitutions.** Predictable — see "Near-homophones" and the lookup tables. Recover in context; only ask if context is empty.
2. **Contextual misfires.** STT falls back to a safe common default when a spoken phrasing doesn't match anything it expects. The transcript is grammatical; the meaning is wrong. Not exhaustively cataloged — when in doubt, stop and ask.

## The recovery form

Whenever any trigger below fires, the response is the same shape: **one short line, freeform, C2 register — never a multi-choice guess list.** The maintainer is a ten-year-experience peer; the agent does not scaffold, gloss, or explain *why* the prompt was unclear. The maintainer rephrases in their own words; the agent continues. This form applies identically to all three stop triggers below, so it isn't repeated per trigger.

## The rule

The discriminator is **whether the requested action applies to the named target**, and whether the prompt has enough specificity to act on at all — not the literal word used. When both are met, the agent acts. When either fails, the agent stops, using the recovery form above.

## Stop triggers

### 1. Action does not apply to target

The literal verb is fine, but the action doesn't apply to the target named. The agent has no license to substitute a verb.

Trigger shape: a verb-noun pair where the verb is a real command but the noun isn't something that verb operates on. Example: `comment the branch` — branches don't take comments. The charitable readings (`commit the branch`, or "leave a review comment on a commit on the branch") both require substituting a verb, which the agent doesn't do.

Example rephrase: `Pausing — "comment the branch": branches do not take comments. Did you mean commit, or a review comment on a specific commit?`

### 2. Prompt is under-specified

No target, no action, or both missing — the agent can imagine *some* target but not *the* target meant. Naming the wrong one costs wasted work plus a retraction, the same error class as inventing a charitable reading of nonsense.

Trigger shape: vague instructions with no concrete verb-target pair — `do something`, `make this happen`, `handle that`, `do the thing`, `fix the obvious one`.

Example rephrase: `Pausing — "do something and then comment": no target named, no concrete action beyond "comment". Which file, which branch, what kind of comment?`

### 3. No charitable reading exists

Grammatically fine, but the meaning matches nothing a competent maintainer would say in context — no plausible target, action, or reading that produces a concrete file-and-line answer.

Trigger shape: nonsense in the maintainer's voice (e.g. a sentence about drinking violet purple milk — not actionable, and inventing a charitable reading would be harmful).

Example rephrase: `Pausing — I did not understand that. Please rephrase.` The agent does not propose interpretations or speculate about the maintainer's state.

## How to stop

Stop with the one-line rephrase, naming the missing or mismatched piece, and wait for the maintainer's next freeform prompt — the stop is a pause, not a quiz.

When the maintainer explicitly asked for a rephrase (see `SKILL.md` → "Rephrase before further work"), the rephrase *is* the entire response: name what's not sure about in one short line if applicable, and stop. No tools that move work forward are called in a rephrase turn.

## Spoken disambiguation

The maintainer knows the STT pipeline rephrases without context, so when a phrasing is likely to get mangled, they attach an extra clause in the same prompt that pins the literal reading and rules out the recovered one. The agent trusts the literal text when this clause is present.

Example: `comment the current branch, in the PR description so the next agent sees the ADR pointer` — the extra clause makes `comment` make sense, so the agent doesn't recover to `commit`. Without the disambiguating clause, trigger 1 fires as normal.

## What not to do

- Do not substitute a verb — `comment the branch` is not a license to commit the branch.
- Do not invent a charitable reading of a nonsense prompt.
- Do not gloss, rephrase, or "just to confirm" ordinary transcripts — the maintainer is a C2-native peer.
- Do not write explanations of *why* the prompt is nonsense into the reply — the recovery is one line.

---

# Lookup tables

Narrow, predictable substitutions the maintainer's dictation pipeline produces. **Not exhaustive** — extend as new patterns surface, and note the addition in the same slice that documents it.

**Scope.** This section covers near-homophone and number-word substitutions only. Action-target mismatches, under-specified prompts, and no-charitable-reading cases are stop triggers, not quirks — see the three sub-rules above.

**Spelled-out acronyms.** The maintainer sometimes spells an acronym out in ICAO letter names; the pipeline can transcribe the spelled-out letters as the whole word they start (e.g. "alpha" → "a"). Treat this as a narrow near-homophone substitution — same recovery shape as any other row below.

## Number words → digits

| Spoken | Maps to |
|---|---|
| one / won | `1` |
| two / to / too | `2` |
| three / free | `3` |
| four / for / fore | `4` |
| five | `5` |
| six | `6` |
| seven | `7` |
| eight / ate | `8` |
| nine / nigh / nein | `9` |
| ten | `10` |

| Spoken | Maps to |
|---|---|
| sixth | `6` |
| seventh | `7` |
| eighth | `8` |
| ninth | `9` |
| tenth | `10` |

Ordinals above "tenth" are rare; STT falls back to spelled-out cardinals. In a numbered-list context, prefer the digit; in prose, the spelled-out form is fine.

## Multi-digit spoken forms

| Spoken | Maps to |
|---|---|
| `double one` | `11` |
| `double two` | `22` |
| `triple one` | `111` |
| `triple nine` | `999` |
| `triple seven` | `777` |
| `one one` | `11` (digit-by-digit) |
| `one two three` | `123` (digit-by-digit) |

For "double N" / "triple N," apply the digit-substitution table to the inner number first, then repeat.

## Slice / step names

| Spoken | Maps to |
|---|---|
| `slice three` | `slice 3` |
| `slice three bravo` | `slice 3B` |
| `slice freebie` | `slice 3B` (digit `3` + letter `B` → "free-bee" → "freebie") |
| `step two` | `step 2` |
| `step two alpha` | `step 2A` |
| `step two able` | `step 2A` (phonetic: "A" → "able") |

Always confirm the mapping inline in the reply — the maintainer shouldn't have to re-derive `bravo → B → 3B` or `freebie → 3B` from the transcript.

### Why `freebie` maps to `3B`

STT fused the digit "free" (three) and the letter name "bee" (B) into the single token `freebie`. Recovery rule: split the token on the `ee` boundary, map each half through the number-word and letter-name tables, concatenate. Same pattern produces `foursea` → `4C`, `fivenee` → `5K`, `atejay` → `8J`, `ninerare` → `9R`. If a token ends in a letter-name sound, treat the head as digit and tail as letter; if both halves are digits, the result is a two-digit number (`freeone` → `31`).

## Near-homophones

| Spoken | Maps to | Action applies to target? |
|---|---|---|
| `slice` | `slide` | Yes (recover in context) |
| `tokyo` (the city) | `tokio` (the Rust async runtime) | Yes in TypeScript context (recover to the language-native async primitive, derive from surrounding code). Without context: stop. |
| `asyncio` (the Python lib) | spelled-out ICAO (`alpha sierra yankee november charlie india oscar`) or close-homophone variants | Yes in Go context (recover to the language-native async primitive: goroutines, channels, `sync`, `errgroup`). Without context: stop. |
| `pnpm` (or spelled-out `papa november papa mike`) | `cargo` | Yes in Rust context (recover to the Rust package manager, derive flags from surrounding `Cargo.toml`). Without context: stop. |
| `cargo` | `poetry` (or the language-native Python package manager, derived from surrounding `pyproject.toml`) | Yes in Python context (recover to the language-native package manager). Without context: stop. |
| `commute` | `commit` | Depends — `commute the branch` is action-target mismatch (trigger 1); `commute the matrix` is fine. |
| `comment` | `commit` | Depends — see note below. |

**Note on `comment` ↔ `commit`.** Not a near-homophone in the action-applies sense when the target is a branch — `comment the branch` fails the discriminator (trigger 1), and the agent stops rather than silently recovering