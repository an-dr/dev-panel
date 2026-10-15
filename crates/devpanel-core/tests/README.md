# Repository-health vectors

`repository-health.vectors.json` is a copy of Commits' conformance vectors for its [repository-health contract](https://github.com/an-dr/commits/blob/v1.9.1/docs/repository-health.md), taken unchanged from the Commits release **v1.9.1** (`apps/commits/bones-adapter/src/health/repository-health.vectors.json`).

Commits owns the file. `health_vectors.rs` runs every case against `Health::of` and checks the verdict list, so any mismatch is a DevPanel bug. To move to a newer contract, copy the file from that Commits release and update the tag here.
