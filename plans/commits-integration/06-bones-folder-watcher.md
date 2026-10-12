# 06 — bones: folder watcher

- **Repository:** bones
- **Depends on:** nothing
- **Needed by:** optional adoption in 10 (Commits) and 16 (DevPanel)
- **Priority:** optional

## Why

Commits refreshes its graph when files in a repository change. DevPanel refreshes only when it starts, when its window regains focus, after an action, or on a timer, so a commit made in a terminal does not show until you switch back. Watching folders and coalescing their events needs no repository. Deciding which changes matter does, for example "`.git/index` changed, so status is stale". That split matches the bones rule.

## What exists

Commits' `crates/watcher`, a native module that:

- watches a repository's worktree and `.git` recursively
- filters non-mutating access events on Linux (BUG-004), so a scan does not trigger a full refresh
- classifies paths and publishes `repo/*` topics

## Architecture

A **leaf library only**, `crates/bones-watch`, depending on std and `notify`, with no bus. Both consumers are native: Commits' `crates/watcher` module keeps its `repo/*` topics and builds on the library, and DevPanel's optional adapter lives in `devpanel-infra`. A generic bus module would have no caller, so it is left out.

## What to build

```rust
pub enum ChangeKind { Created, Modified, Removed }
pub struct Change { pub paths: Vec<PathBuf>, pub kinds: Vec<ChangeKind> }
pub struct Watcher { /* owns the notify watcher and the debounce thread */ }
impl Watcher {
    pub fn new(quiet: Duration, sink: impl Fn(WatchId, Change) + Send + 'static) -> Result<Self, String>;
    pub fn watch(&mut self, path: &Path, recursive: bool) -> Result<WatchId, String>;
    pub fn unwatch(&mut self, id: WatchId);
}
```

1. **Debouncing.** Coalesce bursts into one `Change` per watch after a quiet period: 200 ms by default, set per `Watcher`. A `git checkout` touching thousands of files gives a handful of calls, not thousands. A rename arrives as removed plus created.
2. **The Linux fix.** Drop non-mutating access events before debouncing, as Commits' BUG-004 fix does. Move that code and its test.
3. **No path classification.** The library reports paths. Each app decides what they mean.
4. **Off the caller's thread.** Watching and debouncing run on the library's own thread. `sink` is called there, so a bus module that uses it posts a message and returns.

## Acceptance

- Tests on a temporary folder show that creating, modifying and removing files each produce a change, that a burst of 1000 writes gives few changes, and that unwatching stops them.
- On Linux, reading files produces no changes.
- The crate has a README. `docs/architecture/structure.md` lists it.

## Out of scope

- A bus module. See Architecture.
- Moving Commits' `.git` classification. It stays in Commits, on top of this crate (brief 10).
- Network filesystems and polling fallbacks.
