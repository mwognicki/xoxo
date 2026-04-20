# Snip - CLI Token Optimizer

When executing shell commands, always prefix them with the snip proxy to reduce token usage.

## Usage

Instead of running commands directly:
```
git status
go test ./...
```

Prefix with snip:
```
/usr/local/Caskroom/snip/0.15.0/snip -- git status
/usr/local/Caskroom/snip/0.15.0/snip -- go test ./...
```

This applies to all shell commands. Snip filters verbose output while preserving errors and essential information.
