# ADR 0004 — MCP client architecture for xoxo

- Status: proposed
- Date: 2026-04-25
- Scope: `xoxo-core` config, runtime, tooling registry, future auth/session
  wiring. Interacts with MCP server configuration already added to
  `xoxo-core`.

## Context

`xoxo` is gaining first-class support for external MCP servers. The immediate
goal is not to author MCP servers, but to let `xoxo` act as an MCP **client**
 and consume tools exposed by external servers over:

- `stdio`
- streamable `http`
- legacy `sse`

The workspace now includes `rust-mcp-sdk = 0.9.0` in `xoxo-core`. Local crate
inspection shows that the SDK already provides the client-side transport and
runtime pieces needed for these three paths:

- `StdioTransport::create_with_server_launch(...)`
- `client_runtime::with_transport_options(...)` for streamable HTTP
- `ClientSseTransport::new(...)` for SSE

This makes `rust-mcp-sdk` a strong fit for the MCP wire protocol layer.
However, the same local SDK documentation also states that OAuth support for
**MCP clients** is not implemented yet, even though server-side OAuth support
exists in that project. That means `xoxo` can use the SDK for transport and
protocol handling now, but cannot delegate end-to-end OAuth client flows to it
yet.

At the same time, `xoxo` already has its own stable runtime boundaries:

- config in `xoxo-core`
- tool registration via the internal `Tool` trait and `ToolRegistry`
- agent/tool execution through `ToolContext`

The decision to make now is how to add MCP compatibility without letting the
rest of `xoxo` become tightly coupled to one SDK's runtime types and transport
opinions.

## Decision

`xoxo` will use `rust-mcp-sdk` as the **underlying MCP transport and protocol
engine**, but will keep an internal `xoxo-core` MCP abstraction layer as the
stable integration boundary.

Concretely:

1. `xoxo-core` will own MCP configuration, lifecycle, discovery, caching, and
   xoxo-facing lazy capability access.
2. `rust-mcp-sdk` will be used behind that boundary to connect to MCP servers
   and exchange MCP messages.
3. The rest of `xoxo` will not depend directly on `rust-mcp-sdk` transport,
   runtime, or schema types.
4. OAuth-aware remote servers are part of the design, but authentication
   remains an explicit xoxo-owned concern until the SDK provides client OAuth
   support.
5. MCP capabilities will be exposed lazily through a small discovery and
   invocation surface first, rather than by eagerly registering every remote
   MCP tool as a first-class xoxo tool at startup.

## Why this shape

### 1. We want the SDK's wire support, not its opinions to leak everywhere

`rust-mcp-sdk` already solves the expensive and failure-prone parts of MCP
client implementation:

- transport startup and shutdown
- initialization handshake
- request/response dispatch
- support for stdio, streamable HTTP, and SSE

Reimplementing that in `xoxo` would be unnecessary risk. But using SDK types
as direct dependencies in agent code, tooling code, and config code would make
future refactors expensive and would expose the rest of the workspace to SDK
API churn.

### 2. xoxo must control context growth

Eagerly expanding every MCP tool into the agent-visible tool catalog would
inflate the initial prompt and make startup/discovery cost scale with the
total number of configured MCP tools, even when most are never used.

Recent MCP proxy designs such as `lazy-mcp` show that a lazy pattern is a
better default for large or dynamic MCP fleets: discover only what is needed,
only when it is needed. `xoxo` should borrow that idea without becoming a
general-purpose proxy product.

### 3. xoxo already has its own tool model

`xoxo` tools are represented through the internal `Tool` trait and registered
through `ToolRegistry`. Remote MCP tools need to appear inside that existing
tooling world if and when we decide they deserve first-class promotion. But we
do not need to make that promotion the default integration path. A smaller
lazy MCP surface can be adapted first, and selective promotion can come later.

### 4. OAuth is not ready to outsource

The current SDK version can help us connect to unauthenticated or
header-token-authenticated servers, but it does not yet provide the client
OAuth flow we ultimately want. If `xoxo` owns the auth boundary, we can:

- support token/header injection immediately
- keep the config shape future-friendly
- adopt SDK-native client OAuth later without changing the outer xoxo model

## Architectural rules

### Rule 1 — `xoxo-core` owns the public MCP model

All xoxo-facing MCP concepts live in `xoxo-core`, not in SDK wrapper code
scattered through the workspace:

- configured MCP servers
- resolved transports
- auth intent
- discovered remote tools/resources/prompts
- session lifecycle
- MCP-related errors exposed to xoxo code

This keeps the rest of the workspace talking to xoxo-owned types.

### Rule 2 — MCP connections sit behind a small internal trait

`xoxo-core` should define a small internal adapter trait for active MCP
connections. The exact method list may evolve, but the shape should be close
to:

```rust
#[async_trait]
trait McpConnection: Send + Sync {
    async fn connect(&self) -> Result<(), McpError>;
    async fn list_tools(&self) -> Result<Vec<McpToolDescriptor>, McpError>;
    async fn call_tool(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, McpError>;
}
```

Later extensions may add:

- `list_resources()`
- `read_resource(...)`
- `list_prompts()`
- prompt execution helpers if needed

The point is not the exact method names. The point is that `xoxo` code talks
to `McpConnection`, while the SDK-backed implementation talks to
`rust-mcp-sdk`.

### Rule 3 — one live client per configured MCP server

Remote MCP tools should not each spawn their own MCP session. `xoxo` should
maintain one reusable client/session per configured server, with:

- lazy startup on first use
- shared discovery cache
- shared lifecycle management
- explicit shutdown support

This matters especially for `stdio`, where connection ownership also means
ownership of a child process.

### Rule 4 — MCP is lazy by default

`xoxo` should not eagerly register every remote MCP tool at startup. Instead,
the default agent-facing integration should be a small lazy MCP surface with
operations equivalent to:

- list available MCP servers
- list tools for a specific server
- describe a specific MCP tool
- invoke a specific MCP tool

This keeps prompt footprint and startup work bounded while still making the
full MCP capability set reachable on demand.

### Rule 5 — selective promotion beats automatic promotion

Some MCP tools may eventually deserve promotion into first-class xoxo tools,
but that should be a selective choice, not the default behavior.

Promotion is appropriate only when a tool is:

- high value for common agent workflows
- stable enough that a dedicated wrapper improves UX
- worth the permanent prompt and registry footprint

Until then, generic lazy invocation is the preferred path.

### Rule 6 — resources and prompts are not required for the first slice

The first end-to-end implementation should focus on MCP **tools** only.
Resources and prompts are valuable, but they add surface area in discovery,
representation, and UI. We should only add them after the transport/session
layer is stable.

### Rule 7 — auth is a separate layer in front of the SDK

Until `rust-mcp-sdk` supports OAuth for MCP clients, xoxo should treat
authentication as a pre-transport concern:

- resolve env-backed config values
- obtain or load credentials through xoxo-owned auth code
- inject headers/tokens into the SDK transport options

When SDK-native client OAuth becomes available and proves adequate, the
adapter implementation may switch internally without changing outer xoxo
config or agent/tooling APIs.

## Consequences

### Positive

- We get mature MCP transport/runtime support quickly.
- `stdio`, `http`, and `sse` all map naturally onto SDK transports.
- The rest of `xoxo` remains insulated from SDK-specific APIs.
- We can support auth incrementally instead of blocking on SDK client OAuth.
- Initial MCP integration remains cheap in prompt and startup cost.
- Large or dynamic MCP server fleets remain usable without eager schema bloat.

### Negative

- We now own an adapter layer instead of using the SDK directly everywhere.
- There will be some duplication between SDK schema objects and xoxo-owned
  descriptors.
- Auth remains partially custom until the SDK closes its client OAuth gap.
- Agents will sometimes need a discovery step before invocation instead of
  seeing every remote tool as a first-class tool immediately.

### Accepted tradeoff

This is intentional duplication in service of a stable boundary. The adapter
layer is smaller and cheaper than allowing the entire runtime to depend
directly on SDK-specific concepts.

## Implementation plan

### Phase 1 — runtime boundary and transport wiring

Add a new MCP client module inside `xoxo-core` responsible for:

- resolving `Config.mcp_servers`
- validating transport-specific settings
- constructing SDK-backed clients for `stdio`, `http`, and `sse`
- owning connection lifecycle and shutdown

This phase should stop at "can connect and initialize".

### Phase 2 — lazy discovery surface

Add discovery/caching for remote MCP tools and expose that capability through
a small xoxo-facing lazy MCP surface rather than one wrapper per remote tool.

This phase should stop at:

- list servers
- list tools
- describe tools
- cache discovery results per server

### Phase 3 — generic invocation bridge

Add a generic MCP invocation path that executes a chosen tool on a chosen
server with supplied arguments.

This phase should stop at:

- invoke remote MCP tools end-to-end
- reuse lazy session startup and cached discovery
- keep the agent-visible MCP surface small

### Phase 4 — selective registry integration

Only after the lazy model is working should we decide whether some MCP tools
should be promoted into the normal xoxo tool catalog.

Design requirement: promotion is selective, not automatic, and any promoted
MCP-backed tools must be clearly namespaced to avoid collisions between
servers or between remote and local tool names.

### Phase 5 — auth expansion

Add xoxo-owned auth helpers for remote MCP servers, starting with injected
headers/tokens. OAuth-specific config remains in place, but full OAuth flows
should be implemented only when the surrounding auth design is ready and the
SDK/client boundary is re-evaluated.

### Phase 6 — resources and prompts

After tools are stable, extend the same connection boundary to:

- resources
- resource templates
- prompts

Those should become first-class xoxo capabilities only when we know how they
fit into the agent/runtime UX.

## Non-goals

- Building MCP server-authoring support for xoxo in this slice
- Replacing xoxo's internal tool abstraction with SDK-native abstractions
- Implementing full OAuth client flows immediately
- Exposing resources/prompts before the tool path is working
- Supporting every possible SDK feature before basic MCP tool execution works
- Automatically promoting every remote MCP tool into the normal xoxo tool
  registry

## Open questions

1. Which MCP capabilities deserve first-class promotion into the normal xoxo
   tool catalog, if any?
2. Should `xoxo` eagerly connect to enabled MCP servers at startup, or stay
   lazy until first use?
3. Where should discovery caches live: in memory only, or persisted under the
   xoxo data directory?
4. How should resources and prompts be represented in the xoxo runtime once we
   add them?
5. What exact xoxo-owned auth abstraction should sit between config and SDK
   transport options?
6. How should the agent-visible lazy MCP discovery/invocation tools be named
   and described?

## Follow-up work

- Add `xoxo-core::mcp` runtime modules
- Implement SDK-backed connection adapters for all three transports
- Add lazy MCP discovery and generic invocation tools
- Evaluate selective promotion of high-value MCP tools into xoxo's tooling
  registry
- Design the auth/token lifecycle for OAuth-guarded remote MCP servers
