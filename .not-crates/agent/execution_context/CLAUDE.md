# execution_context module

Agent-scoped execution environment for stateful shell tools.

## Responsibility

This module owns everything that must persist for the lifetime of a single
agent instance and be accessible across multiple tool calls. Currently that
is one thing: a persistent bash session. Future additions (open file handles,
background process registry) belong here too.

## Who gets an execution context

Only agents whose blueprint includes shell tools (`exec`, and future shell-
adjacent tools). The agent runtime checks the resolved `ToolSet` at spawn time
and calls `AgentExecutionContext::new(options)` only when needed. Agents with
purely stateless tools (e.g. only `http_request`) never get one — no bash
process is spawned on their behalf.

## BashOptions and privilege levels

`BashOptions` is the single place where privilege controls are configured.
Current options:

| Field | Default | Effect |
|---|---|---|
| `restricted` | `false` | Full `bash` session; `true` → `rbash` (disables `cd`, `PATH` changes, output redirects, absolute-path commands) |
| `login` | `true` | Spawn as a login shell (`bash --login`) so `/etc/profile` and `~/.bash_profile`/`~/.profile` are sourced; gives access to Rustup (`cargo`/`rustc`) and system-registered Go paths |
| `inherit_env` | `true` | Inherit the parent process environment (including `PATH`, `HOME`, etc.); `false` → clean slate (`env_clear`) |
| `env_vars` | empty | Extra variables applied on top of the inherited env (or the only variables when `inherit_env: false`) |

By default a bash session inherits the full parent environment **and** runs as a
login shell, so tools in non-standard paths (e.g. `/opt/homebrew/bin`,
`~/.cargo/bin`, `/usr/local/go/bin`) are found without any manual configuration.
Set `login: false` for agents that must not execute user-controlled init scripts.
Set `inherit_env: false` for agents that must not observe the parent's environment.

**Important**: `rbash` is not a security boundary on its own. It raises the bar
against casual escapes but must be combined with OS-level controls:
- Kubernetes pod security context (non-root UID, read-only root FS)
- seccomp profile
- Linux namespaces (pid, network, mount)

## Capability elevation

Capability controls (e.g. network access) are **OS-level**, not shell-level.
`rbash` raises the bar against casual escapes but cannot isolate network access
or syscalls — that requires kernel mechanisms.

The elevation model:
1. Agent starts with a restricted `AgentExecutionContext` (e.g. `network_access: false`)
2. The app user is prompted for explicit consent
3. The runtime discards the current context and creates a new one with elevated options
4. The new bash session starts fresh — prior session state is intentionally lost as a safety property

`BashOptions` is the single configuration point for all capability levels.
Future fields map to OS controls (do not add without a corresponding issue — see #29):

| Field | OS mechanism |
|---|---|
| `network_access: bool` | Linux network namespace — bash process has no external routing when `false` |
| `allowed_paths: Option<Vec<PathBuf>>` | chroot / bind mount — restricts filesystem visibility |
| `resource_limits: Option<ResourceLimits>` | cgroup v2 — CPU and memory caps |
| `allowed_syscalls: Option<SeccompProfile>` | seccomp — restricts syscalls available to bash and its children |

In Kubernetes: `network_access` maps to `NetworkPolicy`; resource limits map to
pod `resources.limits`. Pod security context (non-root UID, read-only root FS)
is always applied regardless of `BashOptions`.

## Sentinel protocol

Each command is wrapped server-side before being written to bash stdin:

```sh
{user_cmd}
__ec__=$?
printf '%s\n' "--DONE--$__ec__--"   # stdout sentinel
printf '%s\n' "--DONE--" >&2        # stderr sentinel
```

Both stdout and stderr are drained concurrently via `tokio::select!` until
each stream sees its sentinel. This avoids deadlock when one stream fills its
kernel pipe buffer before the other is read.

## Lifecycle

```
agent spawn
  └─ runtime checks toolset for shell tools
       ├─ yes → AgentExecutionContext::new(options) → BashSession spawned
       └─ no  → ToolContext::execution_context = None

tool call (exec)
  └─ ctx.execution_context.as_ref()? → lock bash → run_command → release lock

agent shutdown (normal or crash/timeout)
  └─ AgentExecutionContext::shutdown() → BashSession::kill() → wait for exit
```

The runtime is responsible for calling `shutdown()`. Tools must never kill
the session themselves.
