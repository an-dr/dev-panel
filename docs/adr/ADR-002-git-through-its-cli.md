# ADR-002: Git is driven through its command line

**Status:** Accepted

## Context

DevPanel could link a git library (libgit2, gitoxide) or run the user's `git` executable.

## Decision

Run `git`. Status comes from `git status --porcelain=v2 --branch --show-stash`, which is a documented, stable machine format; operations are the same commands a user would type.

## Consequences

- DevPanel sees exactly what the user's terminal sees: the same version, config, hooks, credential helpers, SSH keys and LFS. A library would disagree with the CLI in edge cases, and users would trust neither.
- Operation output is git's own, so the Activity log reads like the terminal would.
- `git` must be on `PATH`. A status read costs four short processes, which is why reads go through a bounded worker pool.
- Commands must never wait for input, since no terminal is attached: `GIT_TERMINAL_PROMPT=0`, and SSH runs in batch mode unless the user configured `GIT_SSH_COMMAND`. Credential helpers still work; a missing credential fails instead of hanging.
