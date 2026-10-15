# TODO

## Integrate Commits

Goal: Commits is DevPanel's git client. Any repository opens in it in one click, the same window is reused, and each problem DevPanel reports opens in the Commits view where it is fixed.

The work spans bones, Commits and DevPanel, in that order. It is split into numbered briefs to hand to an agent in each repository: [plans/commits-integration/](plans/commits-integration/README.md). Start with that README; [00-contracts.md](plans/commits-integration/00-contracts.md) holds what the two apps must agree on.

### Where it stands (2026-10-15)

DevPanel's briefs 16 to 19 are done: DevPanel runs on bones 2.0.0, bundles Commits 1.9.1, opens repositories in Commits on the view each problem needs, and publishes its repositories to Commits' selector. [docs/design/commits.md](docs/design/commits.md) describes the result.

DevPanel 0.4.0 ([brief 20](plans/commits-integration/20-devpanel-release.md)) releases it, with the update manifests Settings' update check reads.

### Set aside

- **Downloading Commits on demand, or embedding it as a bones extension,** instead of bundling it: decided in [ADR-003](docs/adr/ADR-003-commits-is-a-bundled-companion.md).
- **DevPanel prompting for credentials** through Commits' askpass helper. Reconsider once the integration ships.
