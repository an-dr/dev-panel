# ADR-003: Commits is a bundled companion, reached only through its contracts

**Status:** Accepted

## Context

Commits is DevPanel's git client: a repository opens in it from its card, and each problem DevPanel reports opens in the Commits view that fixes it. That only works when Commits is installed. Until now it was a default tool pointing at `~/.commits/app/commits`, which worked only for users who had installed Commits on their own.

Commits is a general git client with several callers: a shell, a file manager verb, a VS Code extension, DevPanel. Three ways to make it present were weighed:

- **Download it on demand.** The DevPanel zip stays small, but first use needs the network, and DevPanel would grow its own download-and-verify path.
- **Embed it as a bones extension** inside DevPanel's window. That means two git layers in one process, releases coupled in both directions, and the WASM toolchain in DevPanel's build, all for one panel.
- **Bundle its release.** Every DevPanel release carries Commits' unmodified update archive and manifest, and DevPanel's installer asks bones to install them.

## Decision

Bundle it. Each DevPanel version folder holds `companions/commits-<platform>.json` and the archive that manifest names, both exactly as Commits publishes them for the release pinned by `COMMITS_VERSION` in `.cargo/config.toml`. `devpanel-app --install` installs that archive through the bones upgrade module's companion install, with the policy `IfAbsent`.

DevPanel depends on Commits, never the reverse, and reaches it only through contracts Commits owns and documents: its command line, its theme files, its repository sources and its repository-health vectors. No Commits code is linked or vendored, and no DevPanel file or document is read by Commits.

## Consequences

- DevPanel works with Commits right after installing, offline, and the same way on every machine.
- Each release zip is about 25 MB larger.
- An existing Commits install is never touched, newer or older: Commits updates itself, so the bundled version matters only for a first install. Removing DevPanel leaves Commits installed.
- A newer Commits reaches DevPanel users through Commits' own updater, not through a DevPanel release. Moving the pin only changes what a first install gets.
- A change to a contract starts in Commits' documentation and release; DevPanel then conforms.
