# ADR 0003 — Embeddings and vector search for xoxo

- Status: proposed (seed — to be debated and narrowed before any code lands)
- Date: 2026-04-23
- Scope: `nerd` crate, `xoxo-core` storage and LLM facade, future index crates,
  potentially `concierge`. Interacts with ADR 0001 (agents/subagents) and
  ADR 0002 (deterministic code intelligence).

## Context

ADR 0002 pinned `nerd`'s direction for code intelligence: deterministic
structure first (Tree-sitter ASTs), embeddings second, and editing tools
grounded in concrete facts — never fuzzy matches. That deterministic layer now
exists (`find_symbol`, `find_references`, `inspect_code_structure`,
`patch_symbol`, `rename_symbol`, `ensure_import`, `find_tests_for_symbol`,
`find_patterns`, `find_files`, across 40+ languages via `nerd-ast`).

The gap the agent still has to close by brute force is **discovery**: "where
in this repo is authentication handled?", "is there existing code that does
something like X?", "which tests already cover this area?". Today the agent
either runs many text searches or reads too many files. Both waste tokens,
over-match, and miss intent-level similarity (naming mismatches, renamed
concepts, distributed features).

Embeddings and vector search are the standard answer to this class of
problem. They are also the standard place to accidentally reintroduce
everything the deterministic layer was supposed to eliminate: implicit
edits against stale indexes, answers without citations, cost blow-ups from
re-embedding the world, vendor lock-in on the embedding model, and storage
that quietly diverges from truth.

This ADR is a **seed**, not a final decision. It is meant to frame the
tradeoffs, enumerate the real choices, and propose a default that we can
argue against. Before any implementation work happens, the open questions
below must be settled.

## Non-goals

Pinned up-front so they don't silently creep back in:

- **Embeddings are not an editing authority.** No tool that applies code
  changes ever takes a vector hit as its input. Editing tools keep targeting
  explicit files, symbols, or AST ranges, as in ADR 0002.
- **We are not building a semantic IDE.** LSP-grade precision for cross-file
  refactors is out of scope.
- **We are not building a general RAG framework.** The use cases are
  `xoxo`-internal: code discovery for `nerd`, optionally document/memory
  recall for `concierge`. We do not design around hypothetical third-party
  consumers of the index.
- **We do not chase SOTA retrieval quality.** "Good enough to shortlist
  candidates the agent then verifies" is the bar. Every result is a
  *candidate*, never an answer.
- **No embedding of content the user did not opt into.** In particular, no
  implicit upload of repository content to a hosted embedding provider
  without explicit configuration.

## Guiding principles

These follow from ADR 0002 and from how the rest of `xoxo` is built:

1. **Deterministic structure first, vectors second.** Vector search is a
   ranker over the index the AST layer already gives us — it does not replace
   it. Any answer surfaced to the agent must carry a concrete file, symbol,
   or byte/line range it can verify with deterministic tools.
2. **Everything local by default.** Like the rest of `xoxo`, state lives
   under `~/.xoxo/`. The first-run experience must not require an API key
   for a hosted embedding provider.
3. **Single facade, multiple backends.** Embedding providers sit behind a
   provider-neutral trait in `xoxo-core`, the same way LLM providers do
   (`LlmCompletionRequest`/`Response`). The rest of the workspace never
   imports a vendor-specific embedding type.
4. **Opt-in everything.** Embeddings, each embedding backend, each vector
   store, and each embedding-backed tool are behind Cargo features. The
   default build of `xoxo` does not pull in a vector database or an
   embedding HTTP client.
5. **Transcripts remain the record.** The vector index is a *cache* over
   ground-truth artifacts (source files, persisted chats, memory entries).
   Losing the index must never lose user data. Rebuilds must be deterministic
   from primary storage.
6. **Budget-aware from day one.** Embeddings have a per-token cost and a
   disk/RAM cost. The design treats "how much are we embedding?" as a
   first-class concern, not something to discover after shipping.

## The shape of the decision

There are five orthogonal axes the ADR needs to resolve. Each one has a
proposed default, stated so it can be attacked.

### Axis 1 — What do we actually embed?

Candidates, roughly in order of expected value:

| Content                                  | Primary consumer | Why it matters                                                         |
| ---------------------------------------- | ---------------- | ---------------------------------------------------------------------- |
| Source code chunked by AST symbol         | `nerd`           | Discovery ("where is auth handled?"), similarity ("code like this")    |
| File-level summaries of source files      | `nerd`           | Coarse routing before zooming in via symbol-level vectors              |
| Past chat transcripts (assistant + user)  | `nerd`, `concierge` | "Have we done this before?", continuity across sessions                 |
| Memory / notes (future, ADR not written)  | `concierge`      | Personal-assistant recall                                              |
| Documentation / `docs/` Markdown           | `nerd`           | "What does our ADR say about X?" without re-reading every ADR         |
| Commit messages / PR descriptions          | `nerd`           | "Has this regressed before?" — deferrable                              |

**Proposed default.** Start with **AST-chunked source code** and **file-level
summaries** only. Those are the two that directly serve the discovery gap
ADR 0002 identified but left open. Chat/memory/docs embeddings are a later
scope, gated behind their own feature and ADR.

**Why AST-chunked, not line-windowed.** Symbol-bounded chunks mean every
vector hit already has a file path, symbol name, and byte/line range — the
exact metadata the agent needs to re-enter the deterministic layer and
verify. Line-windowed chunks force a post-processing step to attach any
useful context. Since `nerd-ast` already produces symbol boundaries for 40+
languages, use them.

**Open question.** Do we co-embed *docstrings / leading comments* with the
symbol body, or separately? Cheap gain in retrieval quality, small cost in
chunk complexity. Leaning "co-embed".

### Axis 2 — Which embedding backend(s)?

The LLM facade already teaches us the answer shape: one trait in
`xoxo-core`, many adapters, each behind a feature. Concretely:

```rust
#[async_trait]
pub trait EmbeddingBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn dimensions(&self) -> usize;
    fn max_input_tokens(&self) -> usize;
    async fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>, EmbeddingError>;
}

pub struct Embedding(pub Vec<f32>);
```

Candidate backends:

- **Local, CPU-friendly.** A `fastembed`-style path running a quantised
  sentence-transformer (e.g. `bge-small-en-v1.5`, `all-MiniLM-L6-v2`).
  Default for the zero-config experience. 384-dim or 512-dim is plenty.
- **Local, code-specialised.** A code-tuned model (e.g. `jina-embeddings-v2-base-code`)
  if CPU cost and binary size are acceptable. Higher quality on code
  retrieval, bigger model.
- **OpenAI-compatible HTTP** (`text-embedding-3-small`, `-large`, plus
  anything OpenRouter/Ollama/self-hosted can serve). Mirrors the LLM story:
  users who already have a key get quality upgrades for free.
- **Ollama / llama.cpp native.** Many users already run these locally;
  routing through them costs us one more adapter, not one more deployment.
- **Anthropic.** No first-party embedding model at the time of writing;
  tracked only so we remember to add it if and when that changes.

**Proposed default.** Ship two backends initially: a local `fastembed`-backed
one (feature `embeddings-local`, default when `embeddings` is on) and an
OpenAI-compatible HTTP one (feature `embeddings-openai`). Code-specialised
and Ollama backends are sibling features added when needed. The per-chat
embedding model is resolved the same way LLM model is — a config selection
with runtime override, with a policy enum (`AllowList` / `BlockList` /
`ToolDetermined`) reserved for consistency with the LLM layer.

**Hard constraint.** Switching embedding backends invalidates the index.
Store the backend id + model name + dimensions + tokenizer hash in the index
header; refuse to use an index whose header doesn't match the configured
backend, with a clear rebuild message. Do not silently mix vectors from
different models.

### Axis 3 — Where does the vector store live?

Three realistic options, all local-first:

| Option                                   | Pros                                                      | Cons                                                                                  |
| ---------------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| **sled (already in use)**                | Zero new dep. Atomic with the chat store.                 | No ANN index; we'd hand-roll HNSW or brute-force. Brute force is tolerable up to ~low 10⁵ chunks but breaks down beyond that. |
| **Embedded ANN crate** (e.g. `hnsw_rs`, `instant-distance`) over sled-stored vectors | Fast ANN; persistence stays in sled. | We own the index rebuild + persistence glue.                                          |
| **Embedded vector DB** (e.g. `qdrant` embedded, `lancedb`, `tantivy` + vectors) | Batteries-included. Metadata filters, upserts, snapshots. | Bigger dep, more disk layout opinions, and a second storage engine alongside sled.     |

**Proposed default.** Keep **sled as the durable store of chunk records +
raw vectors**, and layer an **embedded HNSW** (e.g. `hnsw_rs` or
`instant-distance`) over it, rebuildable on startup from sled contents. This
matches the project's "one local state root under `~/.xoxo/data/`" invariant
and avoids a second storage engine. We revisit if/when chunk counts or
metadata-filter complexity outgrow that.

**Index layout sketch.** Inside `~/.xoxo/data/` (extending existing sled
trees):

```
trees/
  embeddings/meta       # backend id, model, dims, tokenizer hash, schema version
  embeddings/chunks     # key: chunk_id (Uuid). value: ChunkRecord (path, lang, symbol, range, hash, backend_id)
  embeddings/vectors    # key: chunk_id. value: [f32; D] as bytes (D from meta)
  embeddings/by_path    # key: path. value: Vec<chunk_id>   (for per-file invalidation)
  embeddings/by_root    # key: repo_root. value: Vec<chunk_id> (for per-repo scope)
```

The ANN index itself is an in-memory structure hydrated from
`embeddings/vectors` at daemon startup. Persisting the HNSW graph to disk is
an optimisation we can add later; rebuilding from vectors is fast compared
to re-embedding.

**Open question.** Do we want *per-repo* indexes (keyed by canonical repo
path) or a single global index with a `repo_root` metadata filter? Per-repo
keeps blast radius small and invalidation cheap. Global lets cross-repo
similarity work. Leaning per-repo for v1.

### Axis 4 — How does indexing stay fresh?

The easiest way to ship a vector index that lies is to index once and never
invalidate. Options:

- **On-demand only.** The agent calls an `index_repo` tool that walks the
  repo, chunks via AST, and embeds missing chunks. Nothing happens in the
  background.
  - *Pro*: simple, transparent, no ghost compute.
  - *Con*: stale by default; agent must remember to refresh.
- **Lazy-on-read with per-file mtime + content-hash check.** A search call
  first checks `by_path` against the working tree; any file whose stored
  hash mismatches gets re-chunked and re-embedded before the search runs.
  - *Pro*: searches never return fully-stale data.
  - *Con*: first search after a big change pays the re-embed cost inline.
- **Background watcher.** A filesystem watcher (`notify`) detects changes
  and re-embeds in the background.
  - *Pro*: searches are always warm.
  - *Con*: ghost compute (and ghost cost) the user didn't ask for;
    complicates shutdown.

**Proposed default.** **On-demand + lazy-on-read**, no background watcher
in v1. The agent gets a `reindex_repo` tool and a `semantic_code_search`
tool; the latter implicitly refreshes any file whose content hash has
changed since its chunks were last embedded. A background watcher is an
opt-in feature (`embeddings-watch`) we add only if users report stale
results dominating their experience.

**Chunk-level invalidation rule.** A chunk is identified by
`(file_path, symbol_path, language, content_hash)`. When a file changes,
recompute its symbol chunks; any chunk whose `(symbol_path, content_hash)`
pair still matches keeps its existing vector. This keeps re-embedding cost
proportional to *what actually changed*, not to file churn.

### Axis 5 — What tools does the agent see?

Deliberately small. Each one returns bounded, structured JSON with concrete
file/symbol/range references (the ADR 0002 contract), and is explicitly
labelled as *candidate discovery* in its description so the agent's policy
is consistent across the toolset.

- `semantic_code_search(query: string, language?: string, scope?: string, k?: int = 10) -> Vec<CodeHit>`
  - Ranks AST-chunked symbols against the natural-language query.
  - Each `CodeHit` includes `file`, `symbol`, `language`, `range`, `score`,
    and a short excerpt. No prose summary.
- `find_similar_code(anchor: { file, symbol } | { file, range }, k?: int = 10) -> Vec<CodeHit>`
  - Given a concrete anchor, return similar chunks elsewhere in the repo.
- `reindex_repo(scope?: string) -> IndexSummary`
  - Walks the repo (or a subpath), AST-chunks it, embeds missing chunks.
  - Returns `{ chunks_added, chunks_refreshed, chunks_skipped, bytes_embedded, model, backend }`.
- *(Deferred)* `semantic_chat_search(query)` — across past chats. Gated
  behind a separate feature once chat embedding is in scope.
- *(Deferred)* `semantic_memory_search(query)` — once `concierge` memory is
  designed.

**Explicitly not added.** No `semantic_patch`, no `edit_like_this`. There is
no tool where vector output drives an edit without the agent going through
a deterministic tool first.

## Proposed crate layout

Extending the ADR 0002 sketch without introducing a new crate unless we
have to:

```
xoxo-core/
  llm/                       # existing: completion facade
  embeddings/                # new
    facade.rs                # EmbeddingBackend trait, request/response shapes
    backends/
      local.rs               # feature = embeddings-local
      openai.rs              # feature = embeddings-openai
      selector.rs            # picks backend from config (mirrors llm/backends/selector.rs)

agentix/
  indexing/                  # new: shared chunk/index contracts if more than one agent uses them
    chunk.rs                 # ChunkRecord, ChunkKind, ContentHash
    vector_store.rs          # trait: upsert, lookup_by_id, nearest, invalidate_by_path
    hnsw_store.rs            # default impl over sled + hnsw_rs

nerd/
  tools/
    semantic_code_search.rs
    find_similar_code.rs
    reindex_repo.rs
  indexing/
    repo_walk.rs             # ignore-aware walker (already a dep)
    chunker.rs               # AST → ChunkRecord via nerd-ast
```

`concierge` would depend on `agentix::indexing` for its own content types
when it reaches that milestone, without touching `nerd`.

## Feature flags

On `xoxo-core`:

- `embeddings` (off by default) — enables the `embeddings` module and the
  `EmbeddingBackend` facade. Nothing in the always-on surface imports from
  it.
- `embeddings-local` (off by default; enabled transitively by `embeddings`
  unless a user picks HTTP-only explicitly) — local CPU backend.
- `embeddings-openai` (off by default) — OpenAI-compatible HTTP backend,
  reusing the `reqwest` dependency already behind the `openai` feature.

On `nerd`:

- `semantic-search` (off by default) — pulls in the three semantic tools and
  the indexing helpers. Requires `xoxo-core/embeddings` at the binary level.

On `xoxo` (the binary):

- `semantic-search` (off by default) — convenience feature that turns on
  `xoxo-core/embeddings` + `nerd/semantic-search` + a sane default backend.

**Invariant, unchanged from CLAUDE.md.** `--no-default-features`, each
backend alone, and `--all-features` all compile. Feature-gated symbols must
not leak into always-on modules.

## Observability and cost

- **Every embedding call goes through the same cost-observability plumbing
  the LLM facade uses.** `CostObservability` records tokens + dollars per
  call; the daemon aggregates per-chat and per-tool.
- **Indexing publishes bus events.** A `reindex_repo` run emits
  `ToolCallStarted` + periodic progress as `TextDelta`-style events (or a
  dedicated payload if we decide progress deserves one — candidate: extend
  `ToolCallKind` with an `IndexProgress` variant, since ADR 0001 explicitly
  holds `ToolCallKind` open for this class of privileged tool).
- **Budget guardrails.** Config section `[embeddings]` declares
  `max_chunks_per_reindex`, `max_total_chunks`, and `per_call_rate_limit`.
  Exceeding them fails the call with a structured error; the agent sees the
  limit and can decide how to scope down.

## Privacy and data boundary

- **Local backend is the default** so the first-run experience does not ship
  repo content to any external service.
- **HTTP backends are strictly opt-in** per provider, configured exactly
  like LLM providers (`[[providers]]` with an `api_key` and a compatibility
  declaration).
- **No automatic telemetry** about what is being indexed. Logs go through
  the existing `log-broadcast` channel and stay local.
- **Per-repo opt-out.** A `.xoxoignore` file (or reusing `.gitignore`
  semantics via the `ignore` crate — already a dep) controls what the
  walker feeds to the embedder. Paths matched by ignore rules are never
  embedded.

## Open questions (must be closed before implementation)

1. **Local embedding model pick.** Which model + which quantisation, and
   what's the binary-size / quality tradeoff we're willing to eat in the
   default build? (Candidates: `bge-small-en-v1.5`, `all-MiniLM-L6-v2`,
   `nomic-embed-text-v1.5`.)
2. **Crate choice for the local runtime.** `fastembed`? Direct `ort` +
   bundled ONNX? `candle`? This is the highest-risk dep choice because it
   decides the binary footprint.
3. **ANN crate pick.** `hnsw_rs` vs. `instant-distance` vs. rolling a
   smaller brute-force cosine scan for v1 and only introducing ANN when
   chunk counts warrant it.
4. **Per-repo vs. global index.** See Axis 3. Decide once, document, stick.
5. **Chunk granularity for long functions / oversized symbols.** Do we
   split a function that exceeds the embedding model's input window, and if
   so, how do we compose retrieval back into a single hit?
6. **Co-embed docstrings with bodies, or separate vectors?** Leaning
   co-embed; needs one quick retrieval-quality experiment to confirm.
7. **Chat-transcript embedding.** In scope for this ADR (gated behind a
   sibling feature) or deferred to its own ADR? Leaning "deferred — separate
   ADR after v1 code search lands".
8. **Background watcher.** In scope for v1 or strictly feature-flagged
   follow-up? Leaning follow-up.
9. **Index schema versioning story.** A `schema_version` in
   `embeddings/meta` is the minimum. Do we also ship a one-shot migration
   step on version bumps, or force a full rebuild?
10. **Multi-workspace handling.** How does indexing behave for users who
    run `xoxo` across multiple repos / `cargo` workspaces in one day?
    Per-repo indexes handle this naturally, but we need to decide how the
    daemon discovers "which repo am I in?" — cwd at daemon start? Per-chat
    scope? Explicit config entry?

## Alternatives considered

- **Embedding-first code intelligence** (skipping or downgrading the AST
  layer). Rejected by ADR 0002 already; reiterated here because it's the
  most common ask and the most common regression risk.
- **Outsourcing the whole index to a hosted vector DB** (Pinecone, Qdrant
  Cloud, etc.). Rejected: conflicts with the "your machine, your data"
  principle; adds a network dependency to a path that should work offline.
- **Hand-rolled flat cosine over sled, forever.** Rejected as *the* answer
  but *accepted* as a viable v0 while we validate chunking and ranking
  quality. ANN is an optimisation we can slot in without reshaping the
  tool contract.
- **Indexing the whole workspace at daemon start.** Rejected: surprise
  compute, surprise cost, surprise disk. On-demand + lazy-on-read keeps the
  user in control.
- **Dedicated `xoxo-index` crate up front.** Rejected for v1 — land the
  contracts in `agentix::indexing` first; promote to a crate if and when a
  second agent or an out-of-process indexer needs them.
- **One giant `ChunkKind` enum covering code, docs, chats, memory from day
  one.** Rejected: overgeneralised before we know what each content type
  actually needs. Start with `CodeSymbol` chunks; extend the enum (not
  mutate) when other types land.

## Milestones (indicative, to be re-scoped after open questions close)

Each milestone leaves the feature matrix green and adds tests. No
sleep-based synchronisation.

### Milestone 1 — Facade and local backend

1. `xoxo-core::embeddings` facade (`EmbeddingBackend`, `Embedding`,
   `EmbeddingError`, request/response types mirroring the LLM facade shape).
2. `embeddings-local` backend: model + tokenizer + inference, no vector
   store yet. Unit test: embed a fixed prompt, assert dimension matches
   declared value, assert deterministic output.
3. `embeddings-openai` backend behind its own feature, reusing the
   existing OpenAI HTTP client. Wired through the same selector shape.

### Milestone 2 — Chunking and storage

1. `agentix::indexing` chunk types and `VectorStore` trait.
2. `HnswStore` over sled: upsert, `nearest(vec, k, filter)`, `invalidate_by_path`,
   `load_from_disk`. No agent-facing tool yet.
3. `nerd::indexing::chunker`: `nerd-ast` → `ChunkRecord` for each supported
   language. Unit tests per language for chunk boundaries and metadata.

### Milestone 3 — Tools and reindex flow

1. `reindex_repo` tool: ignore-aware walk, chunk, embed missing, report
   summary. Honours budget guardrails.
2. `semantic_code_search` tool: lazy-on-read invalidation + HNSW lookup +
   deterministic post-filter (language/scope).
3. `find_similar_code` tool: anchor → query vector → HNSW lookup excluding
   the anchor itself.
4. Integration test: synthetic repo, run `reindex_repo`, assert a known
   query surfaces the expected symbols; mutate a file, rerun search, assert
   the new chunks are returned.

### Milestone 4 — Hardening

1. Index header versioning + refuse-on-mismatch behaviour.
2. Cost observability wired through `CostObservability` + bus.
3. `.xoxoignore` semantics + respect `.gitignore` via `ignore`.
4. Docs update: user-facing section in README, internal reference under
   `crates/lib/xoxo-core/docs/embeddings.md`.

### Milestone 5 (follow-up ADRs, deferred)

- Chat transcript embeddings for `nerd` continuity.
- Memory / notes embeddings for `concierge`.
- Background watcher (`embeddings-watch` feature).
- HNSW graph persistence to avoid cold-start rebuild cost.
- Potential `xoxo-index` crate if a second consumer appears.

## References

- ADR 0001 — Agents and subagents. `ToolCallKind` extension point; bus
  event shape; chat-as-record invariant.
- ADR 0002 — Deterministic code intelligence for `nerd`. AST chunk source;
  "candidates not edits" contract; layering.
- `crates/lib/xoxo-core/docs/bus.md` — bus message reference; where
  indexing progress events fit.
- `crates/agents/nerd-ast/src/languages/` — the 40+ languages we already
  have symbol boundaries for.
- `CLAUDE.md` — project invariants this ADR is bound by (feature matrix,
  `~/.xoxo/` state root, bus-centricity, opt-in everything).
