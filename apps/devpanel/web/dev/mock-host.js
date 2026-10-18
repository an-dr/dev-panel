'use strict';
// A stand-in for the DevPanel host, for working on the page in a plain browser
// (scripts/preview.ps1). It answers the same protocol with canned data and
// simulated jobs. Never part of the app.
(() => {
  const now = Date.now() / 1000;
  const tools = [
    { id: 'vscode', name: 'VS Code', launch: { kind: 'program', program: 'code', args: ['{path}'] } },
    { id: 'claude', name: 'Claude Code', launch: { kind: 'terminal', command: 'claude' } },
    { id: 'commits', name: 'Commits', launch: { kind: 'program', program: '~/.commits/app/commits', args: ['{path}'] } },
    { id: 'terminal', name: 'Terminal', launch: { kind: 'terminal', command: null } },
    { id: 'files', name: 'File manager', launch: { kind: 'file-manager' } },
  ];
  const workspace = {
    repos: [
      { id: 'r1', name: 'bones', path: '/home/me/Code/bones', remote: 'https://github.com/an-dr/bones.git', group: 'engine', install_command: null, favorite: true },
      { id: 'r2', name: 'commits', path: '/home/me/Code/commits', remote: 'git@github.com:an-dr/commits.git', group: 'apps', install_command: null, favorite: false },
      { id: 'r3', name: 'devpanel', path: '/home/me/Code/devpanel', remote: null, group: 'apps', install_command: null, favorite: true },
      { id: 'r4', name: 'agents', path: '/home/me/Code/agents', remote: 'https://github.com/an-dr/agents.git', group: null, install_command: null, favorite: false },
      { id: 'r5', name: 'microlog', path: '/home/me/Code/microlog', remote: 'https://github.com/an-dr/microlog.git', group: 'libs', install_command: null, favorite: false },
      { id: 'r6', name: 'artificial-will-game-v2', path: '/home/me/Code/games/awg2', remote: 'https://github.com/an-dr/artificial-will-game-v2.git', group: 'games', install_command: null, favorite: false },
      { id: 'r7', name: 'old-prototype', path: '/home/me/Code/old-prototype', remote: 'https://github.com/an-dr/old.git', group: null, install_command: null, favorite: false },
    ],
    tools,
    settings: { clone_root: '~/Code', default_tool: 'vscode', auto_fetch_minutes: 0, scan_depth: 3, start_page: 'last', sidebar_order: [], theme: 'system' },
  };
  const base = { files: [], exists: true, is_repo: true, branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, staged: 0, unstaged: 0, untracked: 0, conflicted: 0, stashes: 0, operation: null, has_commits: true, remote_url: 'https://github.com/x/y.git' };
  const commit = (subject, hours) => ({ hash: Math.random().toString(16).slice(2, 9), subject, author: 'Andrei', time: now - hours * 3600 });
  const statuses = {
    r1: { ...base, last_commit: commit('chore(release): bump engine to 1.1.1', 30) },
    r2: { ...base, behind: 3, last_commit: commit('docs: make project documentation welcoming', 200) },
    r3: { ...base, upstream: null, remote_url: null, staged: 1, unstaged: 2, untracked: 2, files: [{ code: 'M.', path: 'apps/devpanel/src/main.rs' }, { code: '.M', path: 'apps/devpanel/web/app.js' }, { code: 'MM', path: 'docs/architecture.md' }, { code: '??', path: 'VERIFY.md' }, { code: '??', path: 'scripts/install.ps1' }], last_commit: commit('feat: dashboard overview', 0.2) },
    r4: { ...base, ahead: 2, last_commit: commit('policy: exempt fully-specified releases', 700) },
    r5: { ...base, ahead: 1, behind: 4, last_commit: commit('fix: ring buffer wrap', 50) },
    r6: { ...base, branch: 'feature/levels', upstream: null, last_commit: commit('wip: level loader', 5), stashes: 2 },
    r7: { ...base, exists: false, is_repo: false, branch: null, upstream: null, has_commits: false, remote_url: null, last_commit: null },
  };
  const ORDER = ['missing', 'not-a-repo', 'conflicted', 'in-progress', 'diverged', 'behind', 'dirty', 'ahead', 'unpublished', 'detached', 'clean'];
  function health(s) {
    if (!s.exists) return 'missing';
    if (!s.is_repo) return 'not-a-repo';
    if (s.conflicted) return 'conflicted';
    if (s.operation) return 'in-progress';
    if (s.ahead && s.behind) return 'diverged';
    if (s.behind) return 'behind';
    if (s.staged + s.unstaged + s.untracked) return 'dirty';
    if (s.ahead) return 'ahead';
    if (!s.upstream && s.remote_url) return 'unpublished';
    return ORDER.at(-1);
  }
  function fixes(s, repo) {
    const h = health(s);
    if (h === 'missing') return repo.remote ? ['reclone'] : [];
    const dirty = s.staged + s.unstaged + s.untracked > 0;
    const out = [];
    if (dirty) out.push('commit-all');
    if (s.ahead && s.behind) { if (!dirty) out.push('sync'); } else if (s.behind) out.push('pull'); else if (s.ahead) out.push('push');
    if (!s.upstream && s.branch && s.remote_url) out.push('publish');
    if (dirty) out.push('stash');
    return out;
  }

  const emit = (events) => setTimeout(() => window.dispatchEvent(new CustomEvent('bones-message', { detail: JSON.stringify(events) })), 30);
  // Unzipped and not yet installed: the Installation panel offers Install.
  let registered = false;
  let installed = null;
  const statusEvent = (id) => ({ type: 'status', repo: id, status: statuses[id], health: health(statuses[id]), fixes: fixes(statuses[id], workspace.repos.find((r) => r.id === id)), web_url: statuses[id].remote_url ? statuses[id].remote_url.replace(/\.git$/, '') : null });
  const workspaceEvent = () => ({ type: 'workspace', workspace: structuredClone(workspace), platform: 'linux', version: '0.0.0-preview', commits: true });
  let job = 0;

  function runJob(repo, title, lines, after, fail) {
    const id = ++job;
    emit([{ type: 'job', job: id, repo, title, state: 'running', error: null }]);
    lines.forEach((line, i) => setTimeout(() => emit([{ type: 'log', job: id, line }]), 250 * (i + 1)));
    setTimeout(() => {
      if (!fail) after?.();
      const events = [{ type: 'job', job: id, repo, title, state: fail ? 'failed' : 'succeeded', error: fail || null }];
      if (repo) events.push(statusEvent(repo));
      emit(events);
    }, 250 * (lines.length + 1));
  }

  const name = (id) => workspace.repos.find((r) => r.id === id).name;
  const handlers = {
    // window.devpanelSelect plays `devpanel <path>`: a select after the state.
    ready: () => emit([workspaceEvent(), ...workspace.repos.map((r) => statusEvent(r.id)), ...(window.devpanelSelect ? [{ type: 'select', repo: window.devpanelSelect }] : [])]),
    refresh: ({ repo }) => emit((repo ? [repo] : workspace.repos.map((r) => r.id)).map(statusEvent)),
    fetch: ({ repo }) => (repo ? [repo] : workspace.repos.filter((r) => statuses[r.id].remote_url).map((r) => r.id))
      .forEach((id) => runJob(id, `Fetch ${name(id)}`, ['$ git fetch --all --prune --progress', 'remote: Counting objects: 100% (12/12)', 'From github.com:an-dr/x'])),
    pull: ({ repo }) => runJob(repo, `Pull ${name(repo)}`, ['$ git pull --ff-only --progress', 'Updating 1a2b3c..4d5e6f', 'Fast-forward', ' 3 files changed'], () => { statuses[repo].behind = 0; }),
    push: ({ repo }) => runJob(repo, `Push ${name(repo)}`, ['$ git push --progress', 'Writing objects: 100% (5/5)', 'To github.com:an-dr/x.git'], () => { statuses[repo].ahead = 0; }),
    sync: ({ repo }) => runJob(repo, `Sync ${name(repo)}`, ['$ git pull --rebase --progress', 'Successfully rebased', '$ git push --progress'], () => { statuses[repo].ahead = 0; statuses[repo].behind = 0; }),
    stash: ({ repo }) => runJob(repo, `Stash ${name(repo)}`, ['$ git stash push --include-untracked', 'Saved working directory'], () => { Object.assign(statuses[repo], { unstaged: 0, untracked: 0, staged: 0 }); statuses[repo].stashes++; }),
    commit: ({ repo, message }) => runJob(repo, `Commit ${name(repo)}`, ['$ git add --all', `$ git commit -m ${message}`, '[main 9f8e7d] ' + message], () => { Object.assign(statuses[repo], { unstaged: 0, untracked: 0, staged: 0 }); statuses[repo].ahead++; statuses[repo].last_commit = commit(message, 0); }),
    fix: ({ repo, fix, message }) => {
      if (fix === 'reclone') return runJob(repo, `Re-clone ${name(repo)}`, ['$ git clone --progress …', 'Receiving objects: 100%'], () => { statuses[repo] = { ...base, last_commit: commit('Initial', 100) }; });
      if (fix === 'publish') return runJob(repo, `Publish branch ${name(repo)}`, ['$ git push --progress -u origin HEAD', 'branch set up to track'], () => { statuses[repo].upstream = `origin/${statuses[repo].branch}`; });
      handlers[{ pull: 'pull', push: 'push', sync: 'sync', stash: 'stash', 'commit-all': 'commit' }[fix]]({ repo, message });
    },
    'open-in-commits': ({ repo, theme }) => emit([{ type: 'toast', level: 'info', message: `(preview) would open ${name(repo)} in Commits, ${theme}` }]),
    open: ({ repo, tool }) => emit([{ type: 'toast', level: 'info', message: `(preview) would open ${name(repo)} in ${tool || 'the default tool'}` }]),
    'add-existing': ({ path }) => {
      const id = `r${workspace.repos.length + 10}`;
      workspace.repos.push({ id, name: path.split(/[\\/]/).pop(), path, remote: null, group: null, install_command: null, favorite: false });
      statuses[id] = { ...base, last_commit: commit('Initial commit', 1) };
      emit([{ type: 'toast', level: 'info', message: `Added ${path}` }, workspaceEvent(), statusEvent(id)]);
    },
    scan: () => runJob(null, 'Scan ~/Code', ['0 new repositories'], () => emit([workspaceEvent()])),
    clone: ({ url }) => runJob(null, `Clone ${url.split('/').pop().replace(/\.git$/, '')}`, ['$ git clone --progress ' + url, 'Cloning into …', 'Receiving objects:  45%', 'Receiving objects: 100% (812/812)', 'Resolving deltas: 100%'], () => handlers['add-existing']({ path: `~/Code/${url.split('/').pop().replace(/\.git$/, '')}` })),
    remove: ({ repo }) => { workspace.repos = workspace.repos.filter((r) => r.id !== repo); emit([workspaceEvent()]); },
    edit: ({ repo, name, group, install_command, web_url }) => { Object.assign(workspace.repos.find((r) => r.id === repo), { name, group: group || null, install_command: install_command || null, web_url: web_url || null }); emit([workspaceEvent()]); },
    'set-group': ({ repo, group }) => { workspace.repos.find((r) => r.id === repo).group = group || null; emit([workspaceEvent()]); },
    favorite: ({ repo, favorite }) => { workspace.repos.find((r) => r.id === repo).favorite = favorite; emit([workspaceEvent()]); },
    export: ({ folder }) => emit([{ type: 'toast', level: 'info', message: `(preview) would write devpanel-repos.json to ${folder}` }]),
    import: ({ folder }) => runJob(null, 'Import repositories', [`${folder}/devpanel-repos.json`, '2 repositories in the list', '  + example (~/Code/example)', '  skip bones (already tracked)', 'cloned 1 of 2'], () => emit([workspaceEvent()])),
    'install-recipes': ({ request }) => emit([{ type: 'reply', request, ok: true, error: null, data: [
      { label: 'install.ps1', command: 'pwsh -NoProfile -ExecutionPolicy Bypass -File install.ps1', source: 'install.ps1' },
      { label: 'cargo install', command: 'cargo install --path .', source: 'Cargo.toml' },
    ] }]),
    run: ({ repo, command, title }) => runJob(repo, title || command, [`$ ${command}`, '   Compiling devpanel v0.1.0', '    Finished release [optimized]'], null, command.includes('fail') ? 'error: build failed' : null),
    'save-settings': ({ settings }) => { workspace.settings = settings; emit([workspaceEvent()]); },
    'save-tools': ({ tools: next }) => { workspace.tools = next; emit([workspaceEvent()]); },
    desktop: ({ request, action }) => {
      if (action !== 'status') registered = action === 'register';
      emit([{ type: 'reply', request, ok: true, error: null, data: registered }]);
    },
    installation: ({ request, action }) => {
      const report = action === 'install' ? ['Installed DevPanel 0.0.0-preview to /home/me/.devpanel/app', 'Installed Commits 1.9.1', 'Registered DevPanel with the desktop'] : undefined;
      if (report) { installed = '0.0.0-preview'; registered = true; }
      setTimeout(() => emit([{ type: 'reply', request, ok: true, error: null, data: { installed, current: false, dir: '/home/me/.devpanel/app', report } }]), report ? 900 : 0);
    },
    update: ({ request, action }) => emit([{ type: 'reply', request, ok: true, error: null, data: action === 'check' ? { available: '0.4.0' } : { staged: '0.4.0' } }]),
    'pick-folder': ({ request }) => emit([{ type: 'reply', request, ok: true, error: null, data: '/home/me/Picked' }]),
    'open-url': ({ url }) => emit([{ type: 'toast', level: 'info', message: `(preview) would open ${url}` }]),
    copy: () => {},
  };

  window.ipc = {
    postMessage(text) {
      const action = JSON.parse(text);
      (handlers[action.type] || (() => console.warn('mock: unhandled', action)))(action);
    },
  };
})();
