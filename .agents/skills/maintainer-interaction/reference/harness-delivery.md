## Harness delivery

The same slice, delivered through different agent harnesses, may need different transport operations. The agent adapts the operation to the harness; the slice's correctness, the commit message format, the conventional-commit body, the post-commit report — these are the contract with the maintainer. The harness is the transport.

### The rule

When a harness behaves differently from the others for a given operation, the agent adapts the operation to the harness, not the other way around.

### Worked case: OpenAI Codex commit and the git identity

OpenAI Codex runs under a managed sandbox. Two quirks matter for `git commit`:

1. **Permission re-prompt.** Codex may refuse the first `git commit` invocation with a sandbox error even when the user has the commit as a named action. The correct response is to reissue the same `git commit` invocation with elevated permissions (`sandbox_permissions: "require_escalated"`, scoped to that one command). Do not stage the change in `/tmp` and ask the maintainer to copy it. Do not invent a different commit path. The retry is cheap; the elevation is the expected way of doing the operation under Codex's profile.
2. **Git identity on retry.** Codex sometimes assumes the sandbox has no git identity configured and, on its own, invents one. The maintainer does not want a fabricated identity on the commit. The fix: once the elevation is granted, the agent has full shell access to the user's machine. The user's `~/.gitconfig` (or whatever identity the maintainer uses) is reachable. Read it, set `user.name` and `user.email` from it (or rely on the existing config), and use that. If `user.name` / `user.email` is genuinely missing, stop and ask the maintainer; do not invent.

The pattern generalizes: when a harness sandbox blocks a step the maintainer has named, elevate and continue with the same operation, using the host's actual configuration. Do not invent defaults, do not stage-and-tell, do not route around the operation.

### What this rule is not

- It is not a license to ignore sandbox errors. If a path is read-only for a reason (skill files, harness-owned directories), the elevation is the right move; substitution of a different path is not. See the sandbox-error case in "Named actions are their own green light" above.
- It is not a reason to skip the commit step in the slice lifecycle. The lifecycle still ends in a commit unless the maintainer has explicitly said otherwise.
- It is not permission to alter commit content to fit a harness. The conventional-commit body the maintainer agreed on is the body; the harness does not get a vote on the message.
