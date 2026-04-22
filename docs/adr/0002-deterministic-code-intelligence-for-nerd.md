# ADR 0002 — Deterministic code intelligence for nerd

- Status: proposed
- Date: 2026-04-22
- Scope: `nerd` crate, future `agentix` shared tooling, language-specific code tooling

## Context

`nerd` is the coding-assistant toolkit for `xoxo`. Its current tool surface is
mostly text- and process-oriented: read files, write files, apply line patches,
search files, execute shell commands, and inspect command/process output. That
is enough for many workflows, but it leaves the agent doing too much code
understanding through raw text search and full-file reads.

Raw text tools are intentionally broad, but they are imprecise for programmatic
questions:

- "Where is this function defined?"
- "Which symbols does this file export?"
- "Add this import only if it is missing."
- "Replace this method body without disturbing nearby code."
- "Find tests near this type."

These questions have deterministic answers in many languages. Treating them as
plain text search problems increases token use, creates over-matches, and makes
patches more fragile. `nerd` should grow code-intelligence tools that return
bounded, boring JSON facts the model can reason over.

Vector embeddings can help with fuzzy discovery, such as "where is auth
handled?" or "find code similar to this flow". They should not become the
primary source of truth for editing. For coding work, deterministic structure
must be the first layer; semantic or vector search can rank candidates, but the
agent must still verify and edit against concrete code facts.

## Decision

`nerd` should add deterministic, language-aware code intelligence before adding
embedding-led code search. The first implementation should be AST-backed where
practical, most likely through Tree-sitter grammars for supported languages.

The tool contract should follow these rules:

- Tools return structured JSON with file paths, symbol names, language,
  byte/line ranges, and compact metadata.
- Tools avoid prose interpretation except for short error messages.
- Tools do not hide edits behind fuzzy matching. Editing tools must target
  explicit files, symbols, ranges, or AST nodes.
- Vector or embedding search results are candidate discovery only. They must
  include concrete file and symbol references, and the agent must verify them
  with deterministic read or AST tools before modifying code.
- Language support is added incrementally. Unsupported languages fall back to
  existing text tools.

The likely layering is:

```text
agentix
  shared agent/tool contracts and reusable support

nerd
  generic coding tools
  ast/
    rust
    typescript
    python
    go
  index/
    symbols
    imports
    embeddings
```

`agentix` may own shared abstractions for tool bindings, registry helpers, or
cross-agent indexing contracts. `nerd` should own coding-specific language
tools and policies.

## Initial Tool Candidates

The first useful tool should be:

```text
inspect_code_structure(file_path)
```

It returns a compact outline of a file:

- imports
- exports
- functions
- classes, structs, enums, traits, interfaces, or equivalent type constructs
- methods
- tests
- line ranges for each reported item

Follow-on deterministic tools:

- `find_symbol(name, language?, root?)`
- `find_references(symbol, scope?)`
- `patch_symbol(file_path, symbol, replacement)`
- `ensure_import(file_path, import_spec)`
- `rename_symbol(file_path, symbol, replacement)`
- `find_tests_for_symbol(file_path, symbol)`

Embedding-backed tools can come later:

- `semantic_code_search(query, language?, root?)`
- `find_similar_code(file_path, symbol_or_range)`

These tools return ranked candidates, not edit authority.

## Consequences

This should improve `nerd` in three ways:

- Lower token use: inspect file structure before reading whole files.
- Safer edits: patch by symbol or AST range instead of fragile line-only
  operations.
- Better navigation: answer programmatic questions through language structure
  instead of broad text search.

The tradeoffs:

- Each supported language needs parser/query maintenance.
- AST facts can be stale if indexes are cached without invalidation.
- Full LSP behavior is out of scope unless a later ADR decides otherwise.
- Embeddings require storage, invalidation, and ranking policy, so they should
  wait until deterministic indexing exists.

The guiding principle is: deterministic structure first, fuzzy retrieval second,
and every result grounded in concrete files and ranges.
