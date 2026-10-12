# TODO

## Integrate Commits

Goal: Commits is DevPanel's git client. Any repository opens in it in one click, the same window is reused, and each problem DevPanel reports opens in the Commits view where it is fixed.

The work spans bones, Commits and DevPanel, in that order. It is split into numbered briefs to hand to an agent in each repository: [plans/commits-integration/](plans/commits-integration/README.md). Start with that README; [00-contracts.md](plans/commits-integration/00-contracts.md) holds what the two apps must agree on.

### Where Commits stands (1.8.1, checked 2026-10-12)

- `commits <path>` opens that repository, or every repository below a folder. Only the first argument is read.
- Each launch starts a new process and window; nothing hands a second launch to the running window.
- It reads unfinished operations, conflicts and submodule status, and shows a ready/busy light. It gives no single verdict for the repository.
- It keeps its own list of recent repositories and knows nothing of DevPanel's list.
- It updates itself from its GitHub releases, and registers itself with Windows and Linux desktops.
- It has `GIT_ASKPASS` and `GIT_EDITOR` helpers and GitHub sign-in. DevPanel runs git with prompts disabled.
- Its themes are Paper and Graphite, nothing like DevPanel's navy and cyan.
- Both apps pin the same bones commit, `ff90d95`.

### Set aside

- **Downloading Commits on demand** instead of bundling it. The zip stays small, but it needs the network and a download-and-verify path in DevPanel.
- **Embedding Commits as a bones extension** inside DevPanel. It means two git layers, coupled releases and the WASM toolchain, all for one panel; the handoff gives most of the benefit.
- **DevPanel prompting for credentials** through Commits' askpass helper. Reconsider once the integration ships.
