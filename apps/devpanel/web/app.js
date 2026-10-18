'use strict';
// DevPanel page. Renders host state and turns clicks into host actions; it owns
// no data of its own beyond what is on screen. Protocol: docs/design/protocol.md.

// ---- bridge ---------------------------------------------------------------

const bridge = (() => {
  let nextRequest = 1;
  const waiting = new Map();
  const listeners = [];

  function send(action) {
    window.ipc.postMessage(JSON.stringify(action));
  }

  /** Sends an action that the host answers with a `reply`; resolves with its data. */
  function request(action) {
    const request = nextRequest++;
    return new Promise((resolve, reject) => {
      waiting.set(request, { resolve, reject });
      send({ ...action, request });
    });
  }

  // Each listener says how much a message changed (RENDER); the batch
  // schedules the largest, rather than rebuilding the page for every batch.
  window.addEventListener('bones-message', (event) => {
    let batch;
    try { batch = JSON.parse(event.detail); } catch { return; }
    let weight = RENDER.none;
    for (const message of Array.isArray(batch) ? batch : [batch]) {
      if (message.type === 'reply' && waiting.has(message.request)) {
        const { resolve, reject } = waiting.get(message.request);
        waiting.delete(message.request);
        message.ok ? resolve(message.data) : reject(new Error(message.error));
        weight = RENDER.full;
        continue;
      }
      listeners.forEach((listener) => { weight = Math.max(weight, listener(message) ?? RENDER.full); });
    }
    scheduleRender(weight);
  });

  return { send, request, on: (listener) => listeners.push(listener) };
})();

// ---- render scheduling ----------------------------------------------------

/** How much a host message changed: nothing, only job output, or anything else. */
const RENDER = { none: 0, output: 1, full: 2 };
/** Rebuilds come at most this often, however fast the host sends batches. */
const FULL_RENDER_MS = 50;
/** Job output alone redraws at most this often; it can stream a line per frame. */
const OUTPUT_RENDER_MS = 250;
let renderTimer = 0;
let renderDue = Infinity;
let lastRender = 0;

/**
 * Renders after the wait a change's weight allows, counted from the last
 * render, so a burst of batches costs one rebuild; `render()` cancels it.
 * Timers rather than animation frames, which WebKit pauses when the window
 * is hidden and headless capture barely runs.
 */
function scheduleRender(weight) {
  if (weight === RENDER.none) return;
  const due = lastRender + (weight === RENDER.full ? FULL_RENDER_MS : OUTPUT_RENDER_MS);
  if (due >= renderDue) return;
  clearTimeout(renderTimer);
  renderDue = due;
  renderTimer = setTimeout(render, Math.max(0, due - performance.now()));
}

// ---- state ----------------------------------------------------------------

const MAX_LOG_LINES = 400;
const MAX_JOBS = 60;

const state = {
  ready: false,
  platform: 'linux',
  workspace: { repos: [], tools: [], settings: {} },
  /** repo id → { status, health, fixes } */
  statuses: new Map(),
  /** job id → { job, repo, title, state, error, lines, started, finished } */
  jobs: new Map(),
  view: load('view', 'overview'),
  layout: load('layout', 'grid'),
  filter: 'all',
  group: null,
  favoritesOnly: false,
  sort: load('sort', 'attention'),
  search: '',
  selected: null,
  expandedJobs: new Set(),
  settingsStale: true,
  /** Whether the start page was applied; it is, once, on the first workspace. */
  started: false,
  /** What is being dragged: `{ kind: 'repo', id }` or `{ kind: 'nav', key, section }`. */
  drag: null,
  /** Desktop registration: `registered` is unknown (null) until the host answers. */
  desktop: { registered: null, busy: false },
  /** The install: `status` is `{installed, current, dir}`, null until the host answers; `done` once this run installed itself. */
  installation: { status: null, busy: false, done: false },
  /** Self-update: a newer release, once found, and the one staged for next start. */
  update: { available: null, staged: null, busy: false, checked: false },
};

function load(key, fallback) {
  try { return localStorage.getItem(`devpanel.${key}`) || fallback; } catch { return fallback; }
}
function remember(key, value) {
  try { localStorage.setItem(`devpanel.${key}`, value); } catch { /* storage unavailable */ }
}

bridge.on((message) => {
  switch (message.type) {
    case 'workspace':
      state.ready = true;
      state.workspace = message.workspace;
      state.platform = message.platform;
      state.version = message.version;
      state.commits = message.commits;
      state.settingsStale = true;
      if (!state.started) {
        state.started = true;
        const start = message.workspace.settings.start_page || 'last';
        goTo(start === 'last' ? load('page', state.view) : start);
      }
      for (const id of [...state.statuses.keys()]) {
        if (!repoById(id)) state.statuses.delete(id);
      }
      if (state.selected && !repoById(state.selected)) closeDrawer();
      applyTheme();
      if (state.installation.status === null && !state.installation.busy) installation('status');
      if (state.desktop.registered === null && !state.desktop.busy) desktop('status');
      if (!state.update.checked) update('check');
      break;
    case 'status': {
      // Focus refreshes and the watcher resend statuses that rarely change.
      const entry = { status: message.status, health: message.health, fixes: message.fixes, web_url: message.web_url };
      const known = state.statuses.get(message.repo);
      if (known && JSON.stringify(known) === JSON.stringify(entry)) return RENDER.none;
      state.statuses.set(message.repo, entry);
      break;
    }
    case 'job': onJob(message); break;
    case 'log': {
      const job = state.jobs.get(message.job);
      if (!job) return RENDER.none;
      job.lines.push(message.line);
      if (job.lines.length > MAX_LOG_LINES) job.lines.splice(0, job.lines.length - MAX_LOG_LINES);
      return RENDER.output;
    }
    case 'toast': toast(message.message, message.level); return RENDER.none;
    // `devpanel <path>`: show what the user pointed at.
    case 'select':
      if (repoById(message.repo)) openDrawer(message.repo);
      break;
  }
  return RENDER.full;
});

function onJob(message) {
  const known = state.jobs.get(message.job);
  if (!known) {
    state.jobs.set(message.job, { ...message, lines: [], started: Date.now(), finished: null });
    while (state.jobs.size > MAX_JOBS) state.jobs.delete(state.jobs.keys().next().value);
    return;
  }
  Object.assign(known, { state: message.state, error: message.error, finished: Date.now() });
  if (message.state === 'failed') {
    toast(`${message.title} failed\n${lastLines(message.error, 4)}`, 'error');
    if (message.repo == null || state.view !== 'activity') state.expandedJobs.add(message.job);
  } else if (message.state === 'succeeded' && message.repo == null) {
    toast(`${message.title} finished`, 'success');
  }
}

const repoById = (id) => state.workspace.repos.find((repo) => repo.id === id);
const toolById = (id) => state.workspace.tools.find((tool) => tool.id === id);
const runningJob = (repoId) => [...state.jobs.values()].reverse().find((j) => j.repo === repoId && j.state === 'running');

// ---- presentation helpers -------------------------------------------------

const HEALTH = {
  missing: { label: 'Missing', tone: 'danger', hint: 'The folder no longer exists' },
  'not-a-repo': { label: 'Not a repository', tone: 'danger', hint: 'The folder holds no git repository' },
  conflicted: { label: 'Conflicts', tone: 'danger', hint: 'Merge conflicts need resolving' },
  'in-progress': { label: 'In progress', tone: 'warn', hint: 'A merge, rebase or similar is unfinished' },
  diverged: { label: 'Diverged', tone: 'warn', hint: 'Local and remote both have new commits' },
  behind: { label: 'Behind', tone: 'info', hint: 'The remote has commits you do not' },
  dirty: { label: 'Uncommitted', tone: 'warn', hint: 'There are uncommitted changes' },
  ahead: { label: 'Unpushed', tone: 'info', hint: 'You have commits the remote does not' },
  detached: { label: 'Detached HEAD', tone: 'warn', hint: 'Not on a branch' },
  unpublished: { label: 'Unpublished', tone: 'info', hint: 'The branch has no upstream yet' },
  clean: { label: 'Clean', tone: 'muted', hint: 'Up to date and nothing to commit' },
};
const HEALTH_ORDER = Object.keys(HEALTH);
const PENDING = { label: 'Checking…', tone: 'muted', hint: 'Reading status' };

const FIX = {
  reclone: { label: 'Re-clone', icon: 'cloudDown' },
  pull: { label: 'Pull', icon: 'arrowDown' },
  sync: { label: 'Sync', icon: 'sync' },
  push: { label: 'Push', icon: 'arrowUp' },
  publish: { label: 'Publish', icon: 'cloudUp' },
  'commit-all': { label: 'Commit…', icon: 'commit' },
  stash: { label: 'Stash', icon: 'archive' },
  'open-in-tool': { label: 'Resolve', icon: 'code' },
};

const FILTERS = [
  { id: 'all', label: 'All', test: () => true },
  { id: 'attention', label: 'Attention', test: (h) => h && h !== 'clean' },
  { id: 'dirty', label: 'Uncommitted', test: (h, s) => s && changes(s) > 0 },
  { id: 'behind', label: 'Behind', test: (h, s) => s && s.behind > 0 },
  { id: 'ahead', label: 'Unpushed', test: (h, s) => s && s.ahead > 0 },
  { id: 'clean', label: 'Clean', test: (h) => h === 'clean' },
];

const TOOL_ICON = { program: 'code', terminal: 'terminal', 'file-manager': 'folder' };

const esc = (text) => String(text ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const changes = (s) => s.staged + s.unstaged + s.untracked + s.conflicted;
const plural = (n, word, many = `${word}s`) => `${n} ${n === 1 ? word : many}`;
const lastLines = (text, n) => String(text || '').split('\n').slice(-n).join('\n');

// ---- name styles ------------------------------------------------------------
// A repository's name is set in one of the bundled display fonts (fonts.js).
// Unless the user chose one with Edit, the style is derived from the id, so it
// stays the same across restarts and machines without being stored.

// Wide or small faces scaled so every name reads at about the same size.
const FONT_SCALE = { 'press-start-2p': 0.62, michroma: 0.8, 'major-mono-display': 0.85, silkscreen: 0.9, audiowide: 0.92, orbitron: 0.92, dseg14: 0.85, vt323: 1.3 };
const WEIGHT_NAMES = { 400: 'Regular', 700: 'Bold', 900: 'Black' };

function hash(text) {
  let h = 0x811c9dc5;
  for (const ch of text) h = Math.imul(h ^ ch.codePointAt(0), 0x01000193);
  // Spread neighbouring ids such as r1 and r2 across the whole range.
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  return (h ^ (h >>> 12)) >>> 0;
}

function fontById(id) {
  return FONTS.find((f) => f.id === id) || null;
}

// The style picked for a repository nobody styled: a font, one of its real
// weights, and italic only where the font has a real italic.
function autoStyle(repo) {
  const h = hash(repo.id);
  const font = FONTS[h % FONTS.length];
  const weight = font.weights[(h >>> 8) % font.weights.length];
  const italic = font.italics.includes(weight) && ((h >>> 16) & 1) === 1;
  return { font: font.id, weight, italic };
}

function nameStyle(repo) {
  return repo.name_style || autoStyle(repo);
}

// Inline style for an element holding the repository's name.
function nameCss(repo) {
  const style = nameStyle(repo);
  const font = fontById(style.font);
  if (!font) return style.italic ? 'font-style:italic' : '';
  return `font-family:'dp-${font.id}',var(--font);font-weight:${style.weight || 400};font-style:${style.italic ? 'italic' : 'normal'};font-size:${FONT_SCALE[font.id] || 1}em`;
}

function repoName(repo, cls = '') {
  return `<span class="repo-name truncate ${cls}" style="${nameCss(repo)}">${esc(repo.name)}</span>`;
}
function ago(seconds) {
  if (!seconds) return '';
  const d = Date.now() / 1000 - seconds;
  if (d < 60) return 'just now';
  const units = [[60, 'minute'], [3600, 'hour'], [86400, 'day'], [604800, 'week'], [2629800, 'month'], [31557600, 'year']];
  let [size, name] = units[0];
  for (const unit of units) if (d >= unit[0]) [size, name] = unit;
  return `${plural(Math.floor(d / size), name)} ago`;
}
function shortPath(path) {
  return String(path).replace(/^\/home\/[^/]+|^\/Users\/[^/]+|^[A-Z]:\\Users\\[^\\]+/, '~');
}
function healthOf(repo) {
  const entry = state.statuses.get(repo.id);
  return entry ? HEALTH[entry.health] : PENDING;
}
function badge(meta, text) {
  return `<span class="badge tone-${meta.tone}" title="${esc(meta.hint)}">${esc(text || meta.label)}</span>`;
}

// ---- rendering ------------------------------------------------------------

const $ = (selector) => document.querySelector(selector);

function render() {
  // Rebuilding the sidebar or a card mid-drag would drop what is dragged or
  // where it is going; the drag's end renders instead.
  if (state.drag) return;
  clearTimeout(renderTimer);
  renderDue = Infinity;
  lastRender = performance.now();
  renderSidebar();
  renderTitle();
  const content = $('#content');
  // Forms keep what the user typed: the settings view is rebuilt only when
  // it is entered or the saved workspace changes, not on every status event.
  const keep = state.view === 'settings' && content.dataset.view === 'settings' && !state.settingsStale;
  if (!keep) {
    const same = content.dataset.view === state.view;
    const scroll = same ? content.scrollTop : 0;
    // Cards rise in when a view is entered, not again on every rebuild of it.
    content.classList.toggle('settled', same);
    content.innerHTML = state.ready ? VIEWS[state.view]() : '<div class="empty"><div class="spinner" style="margin:0 auto"></div></div>';
    content.dataset.view = state.ready ? state.view : '';
    content.scrollTop = scroll;
    state.settingsStale = false;
  }
  renderDrawer();
  hydrateIcons(document);
}

function hydrateIcons(root) {
  root.querySelectorAll('[data-icon]').forEach((el) => {
    el.innerHTML = icon(el.dataset.icon);
    el.removeAttribute('data-icon');
  });
}

function counts() {
  const all = state.workspace.repos;
  const of = (fn) => all.filter((repo) => {
    const entry = state.statuses.get(repo.id);
    return entry && fn(entry.health, entry.status);
  }).length;
  return {
    total: all.length,
    attention: of((h) => h !== 'clean'),
    dirty: of((h, s) => changes(s) > 0),
    behind: of((h, s) => s.behind > 0),
    ahead: of((h, s) => s.ahead > 0),
    broken: of((h) => ['missing', 'not-a-repo', 'conflicted', 'in-progress'].includes(h)),
    clean: of((h) => h === 'clean'),
    running: [...state.jobs.values()].filter((j) => j.state === 'running').length,
  };
}

function groups() {
  const map = new Map();
  for (const repo of state.workspace.repos) {
    const group = repo.group || '';
    map.set(group, (map.get(group) || 0) + 1);
  }
  return [...map.entries()].sort((a, b) => (a[0] === '') - (b[0] === '') || a[0].localeCompare(b[0]));
}

// The app icon (assets/devpanel.svg), redrawn with paths so it needs no font.
const BRAND_MARK = '<svg viewBox="0 0 128 118" aria-hidden="true"><path d="M24 4h80l20 55-20 55H24L4 59z" fill="#191b39" stroke="#9afff5" stroke-width="8" stroke-linejoin="round"/><path d="m38 40 22 19-22 19M66 80h26" fill="none" stroke="#9afff5" stroke-width="10" stroke-linecap="round" stroke-linejoin="round"/></svg>';

function renderSidebar() {
  const c = counts();
  const favorites = state.workspace.repos.filter((r) => r.favorite).length;
  const current = pageKey();
  // Every page in the sidebar can be dragged to reorder it within its section;
  // Favorites and the groups also take a dragged repository.
  const item = (key, section, iconName, label, count, alert) => `
    <button class="nav-item ${current === key ? 'active' : ''}" data-action="page" data-page="${esc(key)}" data-section="${section}">
      ${icon(iconName)}<span class="label truncate">${esc(label)}</span>
      ${count ? `<span class="count ${alert ? 'alert' : ''}">${count}</span>` : ''}
    </button>`;
  const mainCounts = { overview: [c.attention, true], favorites: [favorites], repos: [c.total], activity: [c.running] };
  const mainItems = ordered(MAIN_PAGES.map(([key]) => key)).map((key) => {
    const [, iconName, label] = MAIN_PAGES.find(([k]) => k === key);
    return item(key, 'main', iconName, label, ...mainCounts[key]);
  }).join('');
  const groupKeys = ordered(groups().filter(([g]) => g).map(([g]) => `group:${g}`));
  const groupItems = groupKeys.map((key) => {
    const group = key.slice('group:'.length);
    return item(key, 'groups', 'layers', group, groups().find(([g]) => g === group)[1]);
  }).join('');
  const nav = (view, iconName, label) => `
    <button class="nav-item ${state.view === view ? 'active' : ''}" data-action="page" data-page="${view}">
      ${icon(iconName)}<span class="label">${label}</span>
    </button>`;
  const theme = state.workspace.settings.theme || 'system';
  const next = theme === 'dark' ? 'light' : 'dark';
  const themeToggle = theme === 'system' ? '' : `
    <button class="nav-item" data-action="theme" data-theme="${next}" title="Switch to the ${next} theme">
      ${icon(theme === 'dark' ? 'moon' : 'sun')}<span class="label">${theme === 'dark' ? 'Dark' : 'Light'} theme</span>
    </button>`;
  $('#sidebar').innerHTML = `
    <div class="brand">
      <div class="brand-mark">${BRAND_MARK}</div>
      <div><div class="brand-name">DevPanel</div><div class="brand-sub">Your repositories</div></div>
    </div>
    ${mainItems}
    ${groupItems ? `<div class="nav-label">Groups</div>${groupItems}` : ''}
    <div class="sidebar-footer">
      ${themeToggle}
      ${nav('settings', 'settings', 'Settings')}
    </div>`;
}

// ---- pages and the sidebar's order ---------------------------------------

/** The sidebar's own pages: key, icon, label, in their default order. */
const MAIN_PAGES = [['overview', 'home', 'Overview'], ['favorites', 'star', 'Favorites'], ['repos', 'repo', 'Repositories'], ['activity', 'activity', 'Activity']];

/** The page on screen as a key: a view, `favorites`, or `group:<name>`. */
function pageKey() {
  if (state.view === 'repos' && state.favoritesOnly) return 'favorites';
  if (state.view === 'repos' && state.group !== null) return `group:${state.group}`;
  return state.view;
}

/** Shows the page `key` names; a group that no longer exists falls back to the overview. */
function goTo(key) {
  Object.assign(state, { view: 'overview', group: null, favoritesOnly: false, filter: 'all' });
  if (key === 'favorites') Object.assign(state, { view: 'repos', favoritesOnly: true });
  else if (key.startsWith('group:') && groups().some(([g]) => g && `group:${g}` === key)) Object.assign(state, { view: 'repos', group: key.slice('group:'.length) });
  else if (['overview', 'repos', 'activity', 'settings'].includes(key)) state.view = key;
  remember('page', pageKey());
}

/** `keys` in the user's sidebar order; keys it does not name follow, in their default order. */
function ordered(keys) {
  const order = state.workspace.settings.sidebar_order || [];
  const rank = (key) => (order.includes(key) ? order.indexOf(key) : order.length + keys.indexOf(key));
  return [...keys].sort((a, b) => rank(a) - rank(b));
}

function saveSettings(changes) {
  state.workspace.settings = { ...state.workspace.settings, ...changes };
  state.settingsStale = true;
  bridge.send({ type: 'save-settings', settings: state.workspace.settings });
}

// ---- drag and drop --------------------------------------------------------
//
// A repository dragged onto Favorites is pinned there, and onto a group moves
// into it. A sidebar item dragged within its own section slides into place as
// it goes, and the new order is saved on release.
//
// Pointer events rather than HTML drag and drop: the Linux web view hands
// HTML drags to its own file-drop handling, so the page never saw them land.

const DRAG_THRESHOLD = 5;
/** A press that may become a drag: where it started and what it would move. */
let press = null;
/** Set when a drag ends, so the click the release also produces is ignored. */
let swallowClick = false;

function dropTarget(x, y) {
  const target = document.elementFromPoint(x, y)?.closest('.nav-item[data-page]');
  if (!target || !state.drag) return null;
  const key = target.dataset.page;
  if (state.drag.kind === 'repo') return key === 'favorites' || key.startsWith('group:') ? target : null;
  return target.dataset.section === state.drag.section ? target : null;
}

/** Moves `item` before or after `target`, sliding the items it passes (FLIP). */
function slide(item, target, after) {
  const siblings = [...item.parentElement.querySelectorAll(`.nav-item[data-section="${item.dataset.section}"]`)];
  const before = new Map(siblings.map((el) => [el, el.getBoundingClientRect().top]));
  target.insertAdjacentElement(after ? 'afterend' : 'beforebegin', item);
  for (const el of siblings) {
    // Box coordinates are window pixels; the sidebar is zoomed by the density.
    const shift = (before.get(el) - el.getBoundingClientRect().top) / density();
    if (!shift || el === item) continue;
    el.animate([{ transform: `translateY(${shift}px)` }, { transform: 'none' }], { duration: 160, easing: 'cubic-bezier(.2,.8,.2,1)' });
  }
}

function beginDrag(event) {
  const { source, drag } = press;
  state.drag = drag;
  document.body.classList.add('is-dragging');
  source.classList.add('dragging');
  // The release still arrives when the pointer leaves the window.
  try { source.setPointerCapture(event.pointerId); } catch { /* not capturable */ }
  if (drag.kind === 'repo') {
    const ghost = document.createElement('div');
    ghost.className = 'drag-ghost';
    ghost.innerHTML = `${icon('repo')}<span>${esc(repoById(drag.id)?.name || '')}</span>`;
    document.body.append(ghost);
    state.drag.ghost = ghost;
  }
  moveDrag(event);
}

function moveDrag(event) {
  const { drag } = state;
  if (drag.ghost) drag.ghost.style.transform = `translate(${event.clientX + 12}px, ${event.clientY + 8}px)`;
  document.querySelectorAll('.drop-target').forEach((el) => el.classList.remove('drop-target'));
  const target = dropTarget(event.clientX, event.clientY);
  if (!target) return;
  if (drag.kind === 'repo') {
    target.classList.add('drop-target');
  } else if (target !== press.source) {
    const box = target.getBoundingClientRect();
    slide(press.source, target, event.clientY > box.top + box.height / 2);
  }
}

function endDrag(event, cancelled) {
  const { drag } = state;
  drag.ghost?.remove();
  document.body.classList.remove('is-dragging');
  document.querySelectorAll('.dragging, .drop-target').forEach((el) => el.classList.remove('dragging', 'drop-target'));
  const target = cancelled ? null : dropTarget(event.clientX, event.clientY);
  state.drag = null;
  if (drag.kind === 'repo' && target) {
    const key = target.dataset.page;
    const repo = repoById(drag.id);
    target.animate([{ transform: 'scale(1.06)' }, { transform: 'none' }], { duration: 220, easing: 'ease-out' });
    if (key === 'favorites' && repo && !repo.favorite) {
      bridge.send({ type: 'favorite', repo: repo.id, favorite: true });
      toast(`${repo.name} is a favorite`, 'success');
    } else if (key.startsWith('group:') && repo && repo.group !== key.slice('group:'.length)) {
      const group = key.slice('group:'.length);
      bridge.send({ type: 'set-group', repo: repo.id, group });
      toast(`Moved ${repo.name} to ${group}`, 'success');
    }
  } else if (drag.kind === 'nav' && !cancelled) {
    const section = [...document.querySelectorAll(`.nav-item[data-section="${drag.section}"]`)].map((el) => el.dataset.page);
    const others = (state.workspace.settings.sidebar_order || []).filter((k) => !section.includes(k));
    const order = [...others, ...section];
    if (JSON.stringify(order) !== JSON.stringify(state.workspace.settings.sidebar_order || [])) saveSettings({ sidebar_order: order });
  }
  render();
}

document.addEventListener('pointerdown', (event) => {
  if (event.button !== 0 || state.drag) return;
  // Buttons inside a card keep their own clicks.
  const control = event.target.closest('button, a, input, select, textarea, .split');
  const repo = event.target.closest('[data-drag-repo]');
  const nav = event.target.closest('.nav-item[data-section]');
  if (nav) press = { source: nav, drag: { kind: 'nav', key: nav.dataset.page, section: nav.dataset.section } };
  else if (repo && !(control && repo.contains(control))) press = { source: repo, drag: { kind: 'repo', id: repo.dataset.dragRepo } };
  else return;
  press.x = event.clientX;
  press.y = event.clientY;
});

document.addEventListener('pointermove', (event) => {
  if (!press) return;
  if (!state.drag) {
    if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_THRESHOLD) return;
    beginDrag(event);
  } else {
    moveDrag(event);
  }
  event.preventDefault();
});

document.addEventListener('pointerup', (event) => {
  if (state.drag) {
    swallowClick = true;
    endDrag(event, false);
  }
  press = null;
});

document.addEventListener('pointercancel', (event) => {
  if (state.drag) endDrag(event, true);
  press = null;
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && state.drag) {
    swallowClick = true;
    endDrag(event, true);
    press = null;
  }
}, true);

// The click a drag's release produces is not a click on what was dragged.
document.addEventListener('click', (event) => {
  if (!swallowClick) return;
  swallowClick = false;
  event.stopImmediatePropagation();
  event.preventDefault();
}, true);

function renderTitle() {
  const c = counts();
  const titles = {
    overview: ['Overview', state.ready ? `${plural(c.total, 'repository', 'repositories')} · ${c.attention ? `${c.attention} need attention` : 'all clean'}` : ''],
    repos: [state.favoritesOnly ? 'Favorites' : state.group ?? 'Repositories', plural(visibleRepos().length, 'repository', 'repositories')],
    activity: ['Activity', c.running ? `${plural(c.running, 'job')} running` : 'Output of everything DevPanel ran'],
    settings: ['Settings', 'Tools, folders and behaviour'],
  };
  const [title, subtitle] = titles[state.view];
  $('#page-title').textContent = title;
  $('#page-subtitle').textContent = subtitle;
}

// ---- views ----------------------------------------------------------------

const VIEWS = {
  overview() {
    if (!state.workspace.repos.length) return welcome();
    const c = counts();
    const stat = (value, label, iconName, tone, filter) => `
      <div class="stat tone-${tone}" data-action="filter" data-filter="${filter}">
        <span class="stat-icon">${icon(iconName)}</span>
        <span class="value">${value}</span><span class="label">${label}</span>
      </div>`;
    const favorites = sortRepos(state.workspace.repos.filter((r) => r.favorite), 'attention');
    const attention = sortRepos(state.workspace.repos.filter((r) => {
      const e = state.statuses.get(r.id);
      return e && e.health !== 'clean';
    }), 'attention');
    const recent = sortRepos(state.workspace.repos, 'recent').slice(0, 6);
    return `
      <div class="stats">
        ${stat(c.total, 'Repositories', 'repo', 'muted', 'all')}
        ${stat(c.attention, 'Need attention', 'alert', c.attention ? 'warn' : 'muted', 'attention')}
        ${stat(c.dirty, 'Uncommitted', 'edit', c.dirty ? 'warn' : 'muted', 'dirty')}
        ${stat(c.behind, 'Behind remote', 'arrowDown', c.behind ? 'info' : 'muted', 'behind')}
        ${stat(c.ahead, 'Unpushed', 'arrowUp', c.ahead ? 'info' : 'muted', 'ahead')}
      </div>
      ${favorites.length ? `<div class="section-head"><h2>Favorites</h2><span class="muted">The repositories you pinned</span></div><div class="cards">${favorites.map(card).join('')}</div>` : ''}
      <div class="section-head"><h2>Needs attention</h2><span class="muted">${attention.length ? 'Open one for its suggested fixes' : ''}</span></div>
      ${attention.length ? `<div class="cards">${attention.map(card).join('')}</div>`
        : `<div class="panel" style="display:flex;gap:12px;align-items:center">${badge(HEALTH.clean, 'All clean')}<span class="muted">Every repository is committed, pushed and up to date with what was last fetched.</span></div>`}
      <div class="section-head"><h2>Recently active</h2><span class="muted">By last commit</span></div>
      <div class="cards">${recent.map(card).join('')}</div>`;
  },

  repos() {
    if (!state.workspace.repos.length) return welcome();
    const repos = visibleRepos();
    const segment = FILTERS.map((f) => {
      const n = state.workspace.repos.filter((r) => inGroup(r) && passes(r, f)).length;
      return `<button class="${state.filter === f.id ? 'active' : ''}" data-action="filter" data-filter="${f.id}">${f.label}<span class="n">${n}</span></button>`;
    }).join('');
    const body = !repos.length
      ? `<div class="empty">${icon('search')}<h3>No repositories match</h3><p>Try another filter or search.</p></div>`
      : state.layout === 'grid' ? `<div class="cards">${repos.map(card).join('')}</div>` : `<div class="rows">${repos.map(row).join('')}</div>`;
    return `
      <div class="toolbar">
        <div class="segmented">${segment}</div>
        <span class="spacer"></span>
        <select class="select" data-action="sort" aria-label="Sort">
          ${[['attention', 'Sort: attention first'], ['name', 'Sort: name'], ['recent', 'Sort: recent activity']]
            .map(([v, l]) => `<option value="${v}" ${state.sort === v ? 'selected' : ''}>${l}</option>`).join('')}
        </select>
        <div class="segmented">
          <button class="${state.layout === 'grid' ? 'active' : ''}" data-action="layout" data-layout="grid" title="Cards">${icon('grid')}</button>
          <button class="${state.layout === 'list' ? 'active' : ''}" data-action="layout" data-layout="list" title="List">${icon('list')}</button>
        </div>
      </div>
      ${body}`;
  },

  activity() {
    const jobs = [...state.jobs.values()].reverse();
    if (!jobs.length) return `<div class="empty">${icon('activity')}<h3>Nothing has run yet</h3><p>Clones, pulls, pushes and installs show their output here.</p></div>`;
    return `<div class="jobs">${jobs.map(jobView).join('')}</div>`;
  },

  settings() {
    const s = state.workspace.settings;
    const toolOptions = state.workspace.tools.map((t) => `<option value="${esc(t.id)}" ${s.default_tool === t.id ? 'selected' : ''}>${esc(t.name)}</option>`).join('');
    return `
      <form class="panel" data-form="settings">
        <h2>General</h2><p>Where repositories go and how DevPanel keeps them current.</p>
        <div class="form">
          <label for="s-root">Clone into</label>
          <div class="field-row"><input id="s-root" class="field mono" name="clone_root" value="${esc(s.clone_root)}">
            <button type="button" class="btn" data-action="browse" data-target="#s-root">${icon('folder')}Browse</button></div>
          <span class="hint">New clones go to a folder named after the repository inside this one.</span>
          <label for="s-tool">Default tool</label>
          <select id="s-tool" class="field" name="default_tool">${toolOptions}</select>
          <span class="hint">What the primary “Open” button on every card uses.</span>
          <label for="s-fetch">Fetch automatically</label>
          <select id="s-fetch" class="field" name="auto_fetch_minutes">
            ${[[0, 'Never'], [5, 'Every 5 minutes'], [15, 'Every 15 minutes'], [30, 'Every 30 minutes'], [60, 'Every hour']]
              .map(([v, l]) => `<option value="${v}" ${Number(s.auto_fetch_minutes) === v ? 'selected' : ''}>${l}</option>`).join('')}
          </select>
          <span class="hint">“Behind” is only as fresh as the last fetch.</span>
          <label for="s-depth">Scan depth</label>
          <input id="s-depth" class="field" type="number" min="1" max="8" name="scan_depth" value="${esc(s.scan_depth)}">
          <span class="hint">How many folder levels “Scan a folder” searches.</span>
          <label for="s-theme">Theme</label>
          <select id="s-theme" class="field" name="theme">
            ${[['system', 'Follow the system'], ['light', 'Light'], ['dark', 'Dark']]
              .map(([v, l]) => `<option value="${v}" ${(s.theme || 'system') === v ? 'selected' : ''}>${l}</option>`).join('')}
          </select>
          <span class="hint">A fixed theme adds a light/dark switch to the sidebar.</span>
          <label for="s-density">Density</label>
          <select id="s-density" class="field" name="density">
            ${[['compact', 'Compact'], ['comfortable', 'Comfortable'], ['spacious', 'Spacious']]
              .map(([v, l]) => `<option value="${v}" ${(s.density || 'comfortable') === v ? 'selected' : ''}>${l}</option>`).join('')}
          </select>
          <span class="hint">Scales spacing and text across the whole window.</span>
          <label for="s-start">Start on</label>
          <select id="s-start" class="field" name="start_page">
            ${[['last', 'The page I was on'], ...MAIN_PAGES.map(([k, , l]) => [k, l]), ...groups().filter(([g]) => g).map(([g]) => [`group:${g}`, `Group: ${g}`])]
              .map(([v, l]) => `<option value="${esc(v)}" ${(s.start_page || 'last') === v ? 'selected' : ''}>${esc(l)}</option>`).join('')}
          </select>
          <span class="hint">Drag sidebar items to reorder them${(s.sidebar_order || []).length ? ' — <button type="button" class="link" data-action="reset-sidebar">reset the order</button>' : ''}. Drop a repository on Favorites or a group to put it there.</span>
          <label for="s-names">New repositories</label>
          <select id="s-names" class="field" name="plain_names_for_new_repos">
            ${[['', 'Get a display font picked for them'], ['plain', 'Keep the plain interface font']]
              .map(([v, l]) => `<option value="${v}" ${(s.plain_names_for_new_repos ? 'plain' : '') === v ? 'selected' : ''}>${l}</option>`).join('')}
          </select>
          <span class="hint">Change any repository’s font with Edit.</span>
          <span class="form-label">Version</span>
          <span class="form-value version-row" id="version-row">${versionRow()}</span>
        </div>
        <div class="panel-actions"><button class="btn primary" type="submit">${icon('check')}Save settings</button></div>
      </form>
      <form class="panel" data-form="tools">
        <h2>Tools</h2><p>What a repository can be opened in. <span class="mono">{path}</span> in arguments becomes the repository path. A terminal tool opens a terminal in the repository and runs its command there.</p>
        <table class="tools-table">
          <thead><tr><th style="width:22%">Name</th><th style="width:16%">Kind</th><th>Program / command</th><th style="width:26%">Arguments</th><th></th></tr></thead>
          <tbody id="tool-rows">${state.workspace.tools.map(toolRow).join('')}</tbody>
        </table>
        <div class="panel-actions">
          <button type="button" class="btn" data-action="add-tool">${icon('plus')}Add tool</button>
          <span class="spacer"></span>
          <button class="btn primary" type="submit">${icon('check')}Save tools</button>
        </div>
      </form>
      ${installationPanel()}
      <div class="panel">
        <h2>Transfer</h2><p>Move your repository list to another machine. Export writes a small JSON file; import reads it and clones every repository into a folder you choose.</p>
        <div class="panel-actions">
          <button type="button" class="btn" data-action="export-repos">${icon('download')}Export list…</button>
          <button type="button" class="btn" data-action="import-repos">${icon('upload')}Import &amp; clone…</button>
        </div>
      </div>`;
  },
};

// Install whenever this build is not the installed one; registering only once
// an install exists, since the menu entry starts the installed launcher.
function installationPanel() {
  const { status, busy, done } = state.installation;
  const { registered, busy: registering } = state.desktop;
  const menu = state.platform === 'windows' ? 'Start menu' : 'application menu';
  const dir = status?.dir ? `<span class="mono">${esc(status.dir)}</span>` : 'your user folder';
  let text = 'Checking…';
  if (status && !status.installed) text = 'Not installed';
  else if (status) {
    text = `${esc(status.installed)} installed${status.current ? '' : done ? ', starts from the menu next time' : ', running a copy from elsewhere'}`;
    if (registered !== null) text += ` · ${registered ? 'registered' : 'not registered'}`;
  }
  const buttons = [];
  if (status?.installed) {
    buttons.push(registered
      ? `<button type="button" class="btn" data-action="desktop" data-desktop="unregister" ${registering ? 'disabled' : ''}>${icon('close')}Unregister</button>`
      : `<button type="button" class="btn" data-action="desktop" data-desktop="register" ${registering || registered === null ? 'disabled' : ''}>${icon('check')}Register with system</button>`);
  }
  if (status && !status.current && !done) {
    const label = busy ? 'Installing…' : status.installed ? 'Install this version' : 'Install';
    buttons.push(`<button type="button" class="btn primary" data-action="self-install" ${busy ? 'disabled' : ''}>${icon('download')}${label}</button>`);
  }
  return `
      <div class="panel" id="installation-panel">
        <h2>Installation</h2><p>Installing copies DevPanel to ${dir}, adds it to the ${menu} with “Open in DevPanel” on folders, and installs Commits when it is missing. Register and Unregister add or remove the menu entry of an install.</p>
        <div class="panel-actions center"><span class="status-text">${text}</span><span class="spacer"></span>${buttons.join('')}</div>
      </div>`;
}

/** Asks the host where DevPanel is installed, or installs the running build. */
async function installation(action) {
  state.installation.busy = true;
  refreshInstallationPanel();
  try {
    const answer = await bridge.request({ type: 'installation', action });
    state.installation.status = { installed: answer.installed, current: answer.current, dir: answer.dir };
    for (const line of answer.report || []) toast(line, line.startsWith('warning:') ? 'error' : 'success');
    if (action === 'install') {
      state.installation.done = true;
      desktop('status');
    }
  } catch (error) {
    if (action === 'install') toast(error.message, 'error');
    else state.installation.status = { installed: null, current: false, dir: null };
  }
  state.installation.busy = false;
  refreshInstallationPanel();
}

/** Asks the host to register, unregister, or report the desktop registration. */
async function desktop(action) {
  state.desktop.busy = true;
  refreshInstallationPanel();
  try {
    state.desktop.registered = await bridge.request({ type: 'desktop', action });
  } catch (error) {
    if (action !== 'status') toast(error.message, 'error');
    else state.desktop.registered = false;
  }
  state.desktop.busy = false;
  refreshInstallationPanel();
}

// The version, and a quiet update item beside it once a newer release exists.
function versionRow() {
  const { available, staged, busy } = state.update;
  const version = `<span class="mono">${esc(state.version || 'unknown')}</span>`;
  if (staged) return `${version}<span class="hint">${esc(staged)} is ready and starts next time DevPanel opens</span>`;
  if (available) return `${version}<button type="button" class="link" data-action="update" ${busy ? 'disabled' : ''}>${busy ? 'Downloading…' : `Update to ${esc(available)}`}</button>`;
  return `${version}<button type="button" class="link" data-action="check-update" ${busy ? 'disabled' : ''}>${busy ? 'Checking…' : 'Check for updates'}</button>`;
}

/** `check` runs once per start and on demand; `stage` downloads for the next start. Never installs on its own. */
async function update(action) {
  state.update.busy = true;
  state.update.checked = true;
  refreshVersionRow();
  try {
    const answer = await bridge.request({ type: 'update', action });
    state.update.available = answer.available || null;
    if (answer.staged) state.update.staged = answer.staged;
    if (action === 'check' && !answer.available && state.update.manual) toast('DevPanel is up to date', 'success');
  } catch (error) {
    // A start-up check stays quiet: no network, or no release yet, is not news.
    if (action === 'stage' || state.update.manual) toast(error.message, 'error');
  }
  state.update.busy = false;
  state.update.manual = false;
  refreshVersionRow();
}

function refreshVersionRow() {
  const row = $('#version-row');
  if (row) row.innerHTML = versionRow();
}

// The settings view keeps its forms between renders, so this panel redraws itself.
function refreshInstallationPanel() {
  const panel = $('#installation-panel');
  if (panel) panel.outerHTML = installationPanel();
}

function welcome() {
  return `
    <div class="welcome">
      <h2>Welcome to DevPanel</h2>
      <p>Keep every repository you work on in one place: see which ones need a pull, a push or a commit, fix them in a click, and open them in VS Code, Claude Code or Commits.</p>
      <div class="welcome-actions">
        <button class="welcome-card" data-action="add-repo" data-tab="clone">${icon('cloudDown')}<strong>Clone a repository</strong><span>From GitHub or any git URL</span></button>
        <button class="welcome-card" data-action="add-repo" data-tab="existing">${icon('folder')}<strong>Add a local folder</strong><span>A repository already on disk</span></button>
        <button class="welcome-card" data-action="add-repo" data-tab="scan">${icon('folderSearch')}<strong>Scan a folder</strong><span>Add every repository under it at once</span></button>
      </div>
    </div>`;
}

function inGroup(repo) {
  return state.group === null || (repo.group || '') === state.group;
}
function passes(repo, filter) {
  const entry = state.statuses.get(repo.id);
  return filter.test(entry?.health, entry?.status);
}
function visibleRepos() {
  const filter = FILTERS.find((f) => f.id === state.filter) || FILTERS[0];
  const q = state.search.trim().toLowerCase();
  const repos = state.workspace.repos.filter((repo) => (!state.favoritesOnly || repo.favorite) && inGroup(repo) && passes(repo, filter) && (!q || [
    repo.name, repo.path, repo.group, repo.remote, state.statuses.get(repo.id)?.status.branch,
  ].some((v) => v && String(v).toLowerCase().includes(q))));
  return sortRepos(repos, state.sort, true);
}
// `favFirst` floats pinned repositories to the top of the main list; the
// overview's own sections (attention, recent) pass it off so their order means
// only what their heading says.
function sortRepos(repos, by, favFirst = false) {
  const time = (r) => state.statuses.get(r.id)?.status.last_commit?.time || 0;
  const rank = (r) => { const e = state.statuses.get(r.id); return e ? HEALTH_ORDER.indexOf(e.health) : 99; };
  const byName = (a, b) => a.name.localeCompare(b.name);
  const within = {
    name: byName,
    recent: (a, b) => time(b) - time(a) || byName(a, b),
    attention: (a, b) => rank(a) - rank(b) || byName(a, b),
  }[by] || byName;
  const sorter = favFirst ? (a, b) => (b.favorite ? 1 : 0) - (a.favorite ? 1 : 0) || within(a, b) : within;
  return [...repos].sort(sorter);
}

function statusSummary(s) {
  if (!s || !s.is_repo) return '';
  const parts = [];
  if (s.branch) parts.push(`<span class="chip" title="${esc(s.upstream ? `tracking ${s.upstream}` : 'no upstream')}">${icon('branch')}${esc(s.branch)}</span>`);
  else if (s.has_commits) parts.push(`<span class="chip">${icon('branch')}detached</span>`);
  if (s.ahead) parts.push(`<span class="counter up" title="Commits to push">${icon('arrowUp')}${s.ahead}</span>`);
  if (s.behind) parts.push(`<span class="counter down" title="Commits to pull">${icon('arrowDown')}${s.behind}</span>`);
  const n = changes(s);
  if (n) parts.push(`<span class="counter changes" title="${s.staged} staged, ${s.unstaged} modified, ${s.untracked} untracked">${icon('edit')}${n}</span>`);
  if (s.stashes) parts.push(`<span class="chip" title="Stashed change sets">${icon('archive')}${s.stashes}</span>`);
  return parts.join('');
}

function favStar(repo) {
  return `<button class="btn sm ghost icon-only star ${repo.favorite ? 'on' : ''}" data-action="favorite" data-repo="${esc(repo.id)}" title="${repo.favorite ? 'Remove from favorites' : 'Mark as favorite'}" aria-pressed="${repo.favorite}">${icon('star')}</button>`;
}

function openButton(repo, size = 'sm') {
  const tool = toolById(state.workspace.settings.default_tool) || state.workspace.tools[0];
  if (!tool) return '';
  return `<span class="split">
      <button class="btn ${size} primary" data-action="open" data-repo="${esc(repo.id)}" title="Open in ${esc(tool.name)}">${icon(TOOL_ICON[tool.launch.kind] || 'code')}${esc(tool.name)}</button>
      <button class="btn ${size} primary" data-action="open-menu" data-repo="${esc(repo.id)}" title="Open in…">${icon('chevronDown')}</button>
    </span>`;
}

// "Open in Commits", beside every Open button: the host picks the Commits view
// from the verdict, so a problem opens where it is fixed (docs/design/commits.md).
const NOTHING_TO_OPEN = new Set(['missing', 'not-a-repo']);
function commitsButton(repo, size = 'sm') {
  if (!state.commits || NOTHING_TO_OPEN.has(state.statuses.get(repo.id)?.health)) return '';
  return `<button class="btn ${size} icon-only" data-action="open-in-commits" data-repo="${esc(repo.id)}" title="Open in Commits">${icon('graph')}</button>`;
}

function webButton(repo) {
  const url = webUrl(repo);
  return url ? `<button class="btn sm icon-only" data-action="open-url" data-url="${esc(url)}" title="Open ${esc(url)}">${icon('globe')}</button>` : '';
}

function fixButtons(repo, entry, limit) {
  if (!entry) return '';
  const tone = HEALTH[entry.health].tone;
  return entry.fixes.slice(0, limit).map((fix) =>
    `<button class="btn sm fix tone-${tone === 'ok' ? 'info' : tone}" data-action="fix" data-fix="${fix}" data-repo="${esc(repo.id)}">${icon(FIX[fix].icon)}${FIX[fix].label}</button>`).join('');
}

function card(repo) {
  const entry = state.statuses.get(repo.id);
  const meta = healthOf(repo);
  const s = entry?.status;
  const job = runningJob(repo.id);
  const headline = entry?.health === 'in-progress' && s.operation ? `${s.operation} in progress` : null;
  const commit = s?.last_commit
    ? `<div class="last-commit">${icon('commit')}<span class="truncate">${esc(s.last_commit.subject)}</span><span class="when">${esc(ago(s.last_commit.time))}</span></div>`
    : `<div class="last-commit muted">${esc(meta.hint)}</div>`;
  const activity = job
    ? `<div class="job-line"><span class="spinner"></span><span>${esc(job.title)}</span><span class="mono truncate">${esc(job.lines.at(-1) || '')}</span></div>`
    : commit;
  return `
    <article class="card tone-${meta.tone} ${state.selected === repo.id ? 'selected' : ''}" data-action="select" data-repo="${esc(repo.id)}" data-drag-repo="${esc(repo.id)}">
      <div class="card-head">
        <div class="card-title">
          <div class="name">${repoName(repo)}${repo.group ? `<span class="chip group">${esc(repo.group)}</span>` : ''}</div>
          <div class="path mono truncate" title="${esc(repo.path)}">${esc(shortPath(repo.path))}</div>
        </div>
        ${badge(meta, headline)}
        ${favStar(repo)}
        <button class="btn sm ghost icon-only more" data-action="repo-menu" data-repo="${esc(repo.id)}" title="More actions">${icon('more')}</button>
      </div>
      <div class="meta">${statusSummary(s)}</div>
      ${activity}
      <div class="card-actions">${openButton(repo)}${commitsButton(repo)}${webButton(repo)}</div>
      ${job ? '<div class="progress"></div>' : ''}
    </article>`;
}

function row(repo) {
  const entry = state.statuses.get(repo.id);
  const s = entry?.status;
  const job = runningJob(repo.id);
  return `
    <div class="row ${state.selected === repo.id ? 'selected' : ''}" data-action="select" data-repo="${esc(repo.id)}" data-drag-repo="${esc(repo.id)}">
      <div class="who"><div style="min-width:0"><div class="truncate" style="font-weight:600">${repoName(repo)}</div>
        <div class="mono muted truncate" style="font-size:11.5px">${esc(shortPath(repo.path))}</div></div></div>
      <div class="meta">${statusSummary(s)}</div>
      <div>${job ? `<span class="job-line"><span class="spinner"></span>${esc(job.title.split(' ')[0])}</span>` : badge(healthOf(repo))}</div>
      <div class="muted truncate" style="font-size:12.5px">${s?.last_commit ? `${esc(s.last_commit.subject)} · ${esc(ago(s.last_commit.time))}` : ''}</div>
      <div class="card-actions" style="flex-wrap:nowrap">${openButton(repo)}${commitsButton(repo)}${webButton(repo)}
        ${favStar(repo)}
        <button class="btn sm ghost icon-only" data-action="repo-menu" data-repo="${esc(repo.id)}">${icon('more')}</button></div>
      ${job ? '<div class="progress" style="position:absolute;left:0;right:0;bottom:0"></div>' : ''}
    </div>`;
}

function jobView(job) {
  const tone = { running: 'info', succeeded: 'muted', failed: 'danger' }[job.state];
  const glyph = job.state === 'running' ? '<span class="spinner"></span>'
    : `<span class="state-dot tone-${tone}">${icon(job.state === 'succeeded' ? 'check' : 'close')}</span>`;
  const repo = job.repo && repoById(job.repo);
  const open = state.expandedJobs.has(job.job) || job.state === 'running';
  const elapsed = Math.max(0, Math.round(((job.finished || Date.now()) - job.started) / 1000));
  const lines = job.lines.map((line) => `<span class="${line.startsWith('$ ') ? 'cmd' : ''}">${esc(line)}</span>`).join('\n');
  return `
    <div class="job">
      <div class="job-head" data-action="toggle-job" data-job="${job.job}">
        ${glyph}<span class="title truncate">${esc(job.title)}</span>
        ${repo ? `<span class="chip">${esc(repo.name)}</span>` : ''}
        <span class="time">${new Date(job.started).toLocaleTimeString()} · ${elapsed}s</span>
        ${icon(open ? 'chevronDown' : 'chevronRight')}
      </div>
      ${open ? `<pre class="console">${lines || '<span class="cmd">waiting for output…</span>'}${job.error ? `\n<span class="err">${esc(job.error)}</span>` : ''}</pre>` : ''}
    </div>`;
}

function toolRow(tool) {
  const kind = tool.launch.kind;
  const program = kind === 'program' ? tool.launch.program : kind === 'terminal' ? (tool.launch.command || '') : '';
  const args = kind === 'program' ? joinArgs(tool.launch.args) : '';
  return `
    <tr data-tool="${esc(tool.id)}">
      <td><input class="field" name="name" value="${esc(tool.name)}"></td>
      <td><select class="field" name="kind" data-action="tool-kind">
        ${[['program', 'Program'], ['terminal', 'Terminal'], ['file-manager', 'File manager']]
          .map(([v, l]) => `<option value="${v}" ${kind === v ? 'selected' : ''}>${l}</option>`).join('')}
      </select></td>
      <td><input class="field mono" name="program" value="${esc(program)}" placeholder="${kind === 'terminal' ? 'command to run (optional)' : kind === 'program' ? 'program or path' : '—'}" ${kind === 'file-manager' ? 'disabled' : ''}></td>
      <td><input class="field mono" name="args" value="${esc(args)}" placeholder="{path}" ${kind !== 'program' ? 'disabled' : ''}></td>
      <td><button type="button" class="btn sm ghost icon-only danger" data-action="remove-tool" title="Remove">${icon('trash')}</button></td>
    </tr>`;
}

function joinArgs(args) {
  return (args || []).map((a) => (/[\s"]/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a)).join(' ');
}
function splitArgs(text) {
  const args = [];
  const re = /"((?:[^"\\]|\\.)*)"|(\S+)/g;
  let m;
  while ((m = re.exec(text))) args.push(m[1] !== undefined ? m[1].replace(/\\"/g, '"') : m[2]);
  return args;
}

// ---- drawer -----------------------------------------------------------------

function renderDrawer() {
  const drawer = $('#drawer');
  const repo = state.selected && repoById(state.selected);
  drawer.classList.toggle('open', !!repo);
  drawer.setAttribute('aria-hidden', repo ? 'false' : 'true');
  if (!repo) return;
  const scroll = drawer.querySelector('.drawer-body')?.scrollTop || 0;
  const entry = state.statuses.get(repo.id);
  const s = entry?.status;
  const meta = healthOf(repo);
  const job = runningJob(repo.id);
  const jobs = [...state.jobs.values()].filter((j) => j.repo === repo.id).reverse().slice(0, 5);
  const action = (name, iconName, label, extra = '') =>
    `<button class="btn" data-action="${name}" data-repo="${esc(repo.id)}" ${job ? 'disabled' : ''} ${extra}>${icon(iconName)}${label}</button>`;
  const details = s && s.is_repo ? `
    <dl class="kv">
      <dt>Branch</dt><dd>${esc(s.branch || (s.has_commits ? 'detached HEAD' : 'no commits yet'))}</dd>
      <dt>Upstream</dt><dd>${esc(s.upstream || '—')}${s.upstream ? ` <span class="muted">(${s.ahead} ahead, ${s.behind} behind)</span>` : ''}</dd>
      <dt>Changes</dt><dd>${s.staged} staged · ${s.unstaged} modified · ${s.untracked} untracked${s.conflicted ? ` · <b style="color:var(--danger)">${s.conflicted} conflicted</b>` : ''}</dd>
      <dt>Stashes</dt><dd>${s.stashes}</dd>
      <dt>Last commit</dt><dd>${s.last_commit ? `<span class="mono">${esc(s.last_commit.hash)}</span> ${esc(s.last_commit.subject)}<br><span class="muted">${esc(s.last_commit.author)} · ${esc(ago(s.last_commit.time))}</span>` : '—'}</dd>
      <dt>Remote</dt><dd class="mono">${s.remote_url ? esc(s.remote_url) : '<span class="muted">none</span>'}</dd>
    </dl>` : `<p class="muted">${esc(meta.hint)}.${repo.remote ? ' It can be cloned again from its remote.' : ''}</p>`;
  drawer.innerHTML = `
    <div class="drawer-head">
      <div style="flex:1;min-width:0">
        <h2 class="truncate">${repoName(repo)}</h2>
        <div class="mono muted truncate" title="${esc(repo.path)}">${esc(repo.path)}</div>
        <div class="meta" style="margin-top:8px">${badge(meta)}${repo.group ? `<span class="chip group">${esc(repo.group)}</span>` : ''}</div>
      </div>
      ${favStar(repo)}
      <button class="btn ghost icon-only" data-action="close-drawer" title="Close (Esc)">${icon('close')}</button>
    </div>
    <div class="drawer-body">
      ${entry && entry.fixes.length ? `<section><h3>Suggested</h3><div class="card-actions">${fixButtons(repo, entry, 9)}</div></section>` : ''}
      <section><h3>Open in</h3>${state.commits && !NOTHING_TO_OPEN.has(entry?.health) ? `<button class="btn primary open-commits" data-action="open-in-commits" data-repo="${esc(repo.id)}">${icon('graph')}Open in Commits</button>` : ''}<div class="tool-list">${state.workspace.tools.map((t) =>
        `<button class="btn" data-action="open" data-repo="${esc(repo.id)}" data-tool="${esc(t.id)}">${icon(TOOL_ICON[t.launch.kind] || 'code')}${esc(t.name)}</button>`).join('')}${webUrl(repo) ? `<button class="btn" data-action="open-url" data-url="${esc(webUrl(repo))}" title="${esc(webUrl(repo))}">${icon('globe')}Web page</button>` : ''}</div></section>
      ${s && !s.is_repo ? '' : `<section><h3>Git</h3><div class="action-grid">
        ${action('git', 'cloudDown', 'Fetch', 'data-op="fetch"')}
        ${action('git', 'arrowDown', 'Pull', 'data-op="pull"')}
        ${action('git', 'arrowUp', 'Push', 'data-op="push"')}
        ${action('git', 'sync', 'Sync', 'data-op="sync"')}
        ${action('commit', 'commit', 'Commit…')}
        ${action('git', 'archive', 'Stash', 'data-op="stash"')}
      </div></section>
      <section><h3>Build &amp; install</h3><div class="card-actions">
        ${action('install', 'package', 'Install…')}${action('run', 'terminal', 'Run command…')}
      </div></section>`}
      <section><h3>Status</h3>${details}</section>
      ${s && s.files && s.files.length ? `<section><h3>Changes</h3>${changeList(s)}</section>` : ''}
      ${jobs.length ? `<section><h3>Recent activity</h3><div class="jobs">${jobs.map(jobView).join('')}</div></section>` : ''}
      <section><h3>Repository</h3><div class="card-actions">
        <button class="btn" data-action="edit" data-repo="${esc(repo.id)}">${icon('edit')}Edit…</button>
        <button class="btn" data-action="copy" data-text="${esc(repo.path)}">${icon('copy')}Copy path</button>
        <button class="btn danger" data-action="remove" data-repo="${esc(repo.id)}">${icon('trash')}Remove</button>
      </div></section>
    </div>`;
  drawer.querySelector('.drawer-body').scrollTop = scroll;
}

function changeList(s) {
  const describe = (code) => {
    if (code === '??') return ['new', 'muted'];
    if (code.includes('U')) return ['conflict', 'danger'];
    if (code[0] !== '.' && code[1] !== '.') return ['mixed', 'info'];
    if (code[0] !== '.') return ['staged', 'ok'];
    return ['modified', 'warn'];
  };
  const shown = s.files.map((f) => {
    const [label, tone] = describe(f.code);
    return `<li><span class="file-code tone-${tone}" title="git status ${esc(f.code)}">${label}</span><span class="mono truncate" title="${esc(f.path)}">${esc(f.path)}</span></li>`;
  }).join('');
  const more = changes(s) - s.files.length;
  return `<ul class="files">${shown}</ul>${more > 0 ? `<p class="muted" style="margin-top:6px">…and ${more} more</p>` : ''}`;
}

/** A browsable https URL for the remote, when it is a well-known host. */
// The address set on the repository wins over the one the host derives
// from its remote.
function webUrl(repo) {
  return repo.web_url || state.statuses.get(repo.id)?.web_url || null;
}

function closeDrawer() {
  state.selected = null;
}

// ---- menus ------------------------------------------------------------------

let menuCleanup = null;

function showMenu(anchor, items) {
  closeMenu();
  const root = $('#menu-root');
  root.innerHTML = `<div class="menu" role="menu">${items.map((item) => {
    if (item === '-') return '<hr>';
    if (item.label && !item.action) return `<div class="menu-label">${esc(item.label)}</div>`;
    return `<button class="${item.danger ? 'danger' : ''}" data-index="${items.indexOf(item)}">${icon(item.icon)}${esc(item.text)}</button>`;
  }).join('')}</div>`;
  const menu = root.firstElementChild;
  const rect = anchor.getBoundingClientRect();
  const width = menu.offsetWidth;
  const height = menu.offsetHeight;
  menu.style.left = `${Math.max(8, Math.min(rect.right - width, innerWidth - width - 8))}px`;
  menu.style.top = `${rect.bottom + height + 6 > innerHeight ? Math.max(8, rect.top - height - 6) : rect.bottom + 6}px`;
  menu.addEventListener('click', (event) => {
    const button = event.target.closest('button[data-index]');
    if (!button) return;
    const item = items[Number(button.dataset.index)];
    closeMenu();
    item.action();
  });
  const outside = (event) => { if (!menu.contains(event.target)) closeMenu(); };
  setTimeout(() => document.addEventListener('mousedown', outside), 0);
  menuCleanup = () => document.removeEventListener('mousedown', outside);
}

function closeMenu() {
  if (menuCleanup) menuCleanup();
  menuCleanup = null;
  $('#menu-root').innerHTML = '';
}

function toolMenu(anchor, repoId) {
  showMenu(anchor, [
    { label: 'Open in' },
    ...state.workspace.tools.map((tool) => ({
      text: tool.name, icon: TOOL_ICON[tool.launch.kind] || 'code',
      action: () => bridge.send({ type: 'open', repo: repoId, tool: tool.id }),
    })),
  ]);
}

function repoMenu(anchor, repoId) {
  const repo = repoById(repoId);
  const s = state.statuses.get(repoId)?.status;
  const git = (op) => () => bridge.send({ type: op, repo: repoId });
  const web = webUrl(repo);
  const manage = [
    { text: repo.favorite ? 'Remove from favorites' : 'Mark as favorite', icon: 'star', action: () => toggleFavorite(repoId) },
    { text: 'Copy path', icon: 'copy', action: () => copy(repo.path) },
    ...(web ? [{ text: 'Open web page', icon: 'globe', action: () => bridge.send({ type: 'open-url', url: web }) }] : []),
    { text: 'Edit…', icon: 'edit', action: () => editModal(repoId) },
    { text: 'Remove from DevPanel', icon: 'trash', danger: true, action: () => removeModal(repoId) },
  ];
  if (s && !s.is_repo) {
    // Nothing git can act on: offer only what still makes sense.
    const reclone = repo.remote ? [{ text: 'Clone again', icon: 'cloudDown', action: () => bridge.send({ type: 'fix', repo: repoId, fix: 'reclone' }) }] : [];
    return showMenu(anchor, [...reclone, { text: 'Refresh status', icon: 'refresh', action: () => bridge.send({ type: 'refresh', repo: repoId }) }, '-', ...manage]);
  }
  showMenu(anchor, [
    { text: 'Fetch', icon: 'cloudDown', action: git('fetch') },
    { text: 'Pull', icon: 'arrowDown', action: git('pull') },
    { text: 'Push', icon: 'arrowUp', action: git('push') },
    { text: 'Sync (pull --rebase, push)', icon: 'sync', action: git('sync') },
    { text: 'Commit all…', icon: 'commit', action: () => commitModal(repoId) },
    { text: 'Stash changes', icon: 'archive', action: git('stash') },
    '-',
    { text: 'Install…', icon: 'package', action: () => installModal(repoId) },
    { text: 'Run command…', icon: 'terminal', action: () => runModal(repoId) },
    { text: 'Refresh status', icon: 'refresh', action: () => bridge.send({ type: 'refresh', repo: repoId }) },
    '-',
    ...manage,
  ]);
}

function copy(text) {
  bridge.send({ type: 'copy', text });
  toast('Copied to the clipboard', 'success');
}

// Flip the mark locally so the star reacts at once; the host answers with a
// workspace event that confirms it.
function toggleFavorite(repoId) {
  const repo = repoById(repoId);
  if (!repo) return;
  repo.favorite = !repo.favorite;
  bridge.send({ type: 'favorite', repo: repoId, favorite: repo.favorite });
  render();
}

// ---- modals -------------------------------------------------------------------

function modal({ title, body, actions, onMount, wide }) {
  const root = $('#modal-root');
  root.innerHTML = `
    <div class="backdrop">
      <div class="modal" role="dialog" aria-modal="true" ${wide ? 'style="width:min(720px,94vw)"' : ''}>
        <div class="modal-head"><h2>${esc(title)}</h2><button class="btn ghost icon-only sm" data-close>${icon('close')}</button></div>
        <div class="modal-body">${body}</div>
        <div class="modal-foot">${actions.map((a, i) => `<button class="btn ${a.kind || ''}" data-modal-action="${i}">${a.icon ? icon(a.icon) : ''}${esc(a.text)}</button>`).join('')}</div>
      </div>
    </div>`;
  const backdrop = root.firstElementChild;
  const close = () => { root.innerHTML = ''; };
  backdrop.addEventListener('mousedown', (e) => { if (e.target === backdrop) close(); });
  backdrop.querySelector('[data-close]').addEventListener('click', close);
  backdrop.querySelectorAll('[data-modal-action]').forEach((button) => {
    button.addEventListener('click', async () => {
      const action = actions[Number(button.dataset.modalAction)];
      if (!action.run) return close();
      const keepOpen = await action.run(backdrop);
      if (!keepOpen) close();
    });
  });
  backdrop.addEventListener('keydown', (e) => {
    const primary = actions.findIndex((a) => a.kind === 'primary');
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey || e.target.tagName === 'INPUT') && primary >= 0) {
      e.preventDefault();
      backdrop.querySelector(`[data-modal-action="${primary}"]`).click();
    }
  });
  if (onMount) onMount(backdrop);
  const first = backdrop.querySelector('input:not([type=hidden]), textarea, select');
  if (first) setTimeout(() => first.focus(), 30);
  return backdrop;
}

const modalOpen = () => !!$('#modal-root').firstElementChild;
const value = (root, selector) => root.querySelector(selector).value;

async function browse(input) {
  try {
    const path = await bridge.request({ type: 'pick-folder' });
    if (path) input.value = path;
  } catch (error) {
    toast(`Could not open the folder picker: ${error.message}`, 'error');
  }
}

function addRepoModal(tab = 'clone') {
  const root = state.workspace.settings.clone_root || '';
  const panes = {
    clone: `
      <span class="label">Repository URL</span>
      <input class="field mono" id="m-url" placeholder="https://github.com/owner/name.git  or  git@github.com:owner/name">
      <span class="label">Into folder</span>
      <div class="field-row"><input class="field mono" id="m-parent" value="${esc(root)}"><button class="btn" data-browse="#m-parent">${icon('folder')}Browse</button></div>
      <span class="note">It is cloned into a new folder named after the repository. Output appears in Activity.</span>`,
    existing: `
      <span class="label">Repository folder</span>
      <div class="field-row"><input class="field mono" id="m-path" placeholder="~/Code/my-project"><button class="btn" data-browse="#m-path">${icon('folder')}Browse</button></div>
      <span class="note">The folder must be the top of a git working tree.</span>`,
    scan: `
      <span class="label">Folder to scan</span>
      <div class="field-row"><input class="field mono" id="m-scan" value="${esc(root)}"><button class="btn" data-browse="#m-scan">${icon('folder')}Browse</button></div>
      <span class="note">Every repository up to ${esc(state.workspace.settings.scan_depth)} levels deep is added; ones already tracked are skipped. Dependency folders such as node_modules are ignored.</span>`,
  };
  const tabs = [['clone', 'cloudDown', 'Clone'], ['existing', 'folder', 'Local folder'], ['scan', 'folderSearch', 'Scan folder']];
  let current = tab;
  const backdrop = modal({
    title: 'Add repository',
    body: `<div class="tabs">${tabs.map(([id, i, l]) => `<button data-tab="${id}" class="${id === tab ? 'active' : ''}">${icon(i)}${l}</button>`).join('')}</div><div id="m-pane">${panes[tab]}</div>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Add', kind: 'primary', icon: 'plus',
        run: (root) => {
          if (current === 'clone') {
            const url = value(root, '#m-url').trim();
            if (!url) return toast('Enter a repository URL', 'error'), true;
            bridge.send({ type: 'clone', url, parent: value(root, '#m-parent') });
            toast('Cloning… follow it in Activity', 'info');
          } else if (current === 'existing') {
            const path = value(root, '#m-path').trim();
            if (!path) return toast('Choose a folder', 'error'), true;
            bridge.send({ type: 'add-existing', path });
          } else {
            bridge.send({ type: 'scan', path: value(root, '#m-scan') });
          }
          return false;
        },
      },
    ],
  });
  const wire = () => backdrop.querySelectorAll('[data-browse]').forEach((b) =>
    b.addEventListener('click', () => browse(backdrop.querySelector(b.dataset.browse))));
  wire();
  backdrop.querySelectorAll('[data-tab]').forEach((button) => button.addEventListener('click', () => {
    current = button.dataset.tab;
    backdrop.querySelectorAll('[data-tab]').forEach((b) => b.classList.toggle('active', b === button));
    backdrop.querySelector('#m-pane').innerHTML = panes[current];
    wire();
    backdrop.querySelector('#m-pane input')?.focus();
  }));
}

function commitModal(repoId) {
  const repo = repoById(repoId);
  const s = state.statuses.get(repoId)?.status;
  modal({
    title: `Commit all changes in ${repo.name}`,
    body: `
      ${s ? `<div class="meta">${statusSummary(s)}<span class="muted">${s.staged} staged · ${s.unstaged} modified · ${s.untracked} untracked</span></div>` : ''}
      <span class="label">Message</span>
      <textarea class="field" id="m-message" placeholder="Describe the change"></textarea>
      <span class="note">Stages everything — including untracked files — and commits it on ${esc(s?.branch || 'the current branch')}. Ctrl+Enter commits.</span>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Commit', kind: 'primary', icon: 'commit',
        run: (root) => {
          const message = value(root, '#m-message').trim();
          if (!message) return toast('A commit message is required', 'error'), true;
          bridge.send({ type: 'commit', repo: repoId, message });
          return false;
        },
      },
    ],
  });
}

async function installModal(repoId) {
  const repo = repoById(repoId);
  let recipes = [];
  try {
    recipes = await bridge.request({ type: 'install-recipes', repo: repoId });
  } catch (error) {
    return toast(error.message, 'error');
  }
  const list = recipes.length
    ? recipes.map((r, i) => `
        <div class="recipe">${icon('package')}
          <div class="what"><strong>${esc(r.label)}</strong><span class="mono truncate" title="${esc(r.command)}">${esc(r.command)}</span></div>
          <button class="btn sm primary" data-recipe="${i}">${icon('play')}Run</button>
        </div>`).join('')
    : '<p class="muted">No install script or build file was recognised in this repository.</p>';
  const backdrop = modal({
    title: `Install ${repo.name}`,
    wide: true,
    body: `
      <span class="note">Found in the repository, best match first. Each runs in the repository folder; output appears in its activity.</span>
      <div>${list}</div>
      <span class="label">Or run a custom command</span>
      <input class="field mono" id="m-custom" value="${esc(repo.install_command || '')}" placeholder="e.g. pwsh ./scripts/install.ps1 -Release">
      <span class="note">Save it as this repository’s install command with Edit… to have it listed first next time.</span>`,
    actions: [
      { text: 'Close' },
      {
        text: 'Run custom', kind: 'primary', icon: 'play',
        run: (root) => {
          const command = value(root, '#m-custom').trim();
          if (!command) return toast('Enter a command', 'error'), true;
          bridge.send({ type: 'run', repo: repoId, command, title: `Install ${repo.name}` });
          openDrawer(repoId);
          return false;
        },
      },
    ],
  });
  backdrop.querySelectorAll('[data-recipe]').forEach((button) => button.addEventListener('click', () => {
    const recipe = recipes[Number(button.dataset.recipe)];
    bridge.send({ type: 'run', repo: repoId, command: recipe.command, title: `Install ${repo.name} (${recipe.label})` });
    $('#modal-root').innerHTML = '';
    openDrawer(repoId);
    render();
  }));
}

function runModal(repoId) {
  const repo = repoById(repoId);
  modal({
    title: `Run in ${repo.name}`,
    body: `
      <span class="label">Command</span>
      <input class="field mono" id="m-command" placeholder="${state.platform === 'windows' ? 'dir' : 'ls -la'}">
      <span class="note">Runs through ${state.platform === 'windows' ? 'cmd' : 'sh'} in ${esc(shortPath(repo.path))}. It cannot ask for input; use a terminal tool for interactive programs.</span>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Run', kind: 'primary', icon: 'play',
        run: (root) => {
          const command = value(root, '#m-command').trim();
          if (!command) return true;
          bridge.send({ type: 'run', repo: repoId, command });
          openDrawer(repoId);
          return false;
        },
      },
    ],
  });
}

function editModal(repoId) {
  const repo = repoById(repoId);
  const known = groups().map(([g]) => g).filter(Boolean);
  modal({
    title: `Edit ${repo.name}`,
    body: `
      <span class="label">Name</span><input class="field" id="m-name" value="${esc(repo.name)}">
      <span class="label">Group</span><input class="field" id="m-group" list="m-groups" value="${esc(repo.group || '')}" placeholder="e.g. work, tools, games">
      <datalist id="m-groups">${known.map((g) => `<option value="${esc(g)}">`).join('')}</datalist>
      <span class="label">Install command</span><input class="field mono" id="m-install" value="${esc(repo.install_command || '')}" placeholder="Leave empty to use what DevPanel detects">
      <span class="label">Name font</span>
      <div class="field-row name-style">
        <select class="field" id="m-font">${fontOptions(repo)}</select>
        <select class="field" id="m-weight"></select>
        <select class="field" id="m-italic"><option value="">Upright</option><option value="1">Italic</option></select>
      </div>
      <div class="name-preview" id="m-preview"></div>
      <span class="label">Web page</span><input class="field mono" id="m-web" value="${esc(repo.web_url || '')}" placeholder="${esc(state.statuses.get(repoId)?.web_url || 'https://…')}">
      <span class="note">Leave empty to use the page DevPanel derives from the remote.</span>
      <span class="note">Path: <span class="mono">${esc(repo.path)}</span></span>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Save', kind: 'primary', icon: 'check',
        run: (root) => {
          bridge.send({
            type: 'edit', repo: repoId,
            name: value(root, '#m-name'), group: value(root, '#m-group'), install_command: value(root, '#m-install'),
              web_url: value(root, '#m-web'),
              name_style: editedStyle(repo, root),
          });
          return false;
        },
      },
    ],
    onMount: (root) => syncNameStyle(repo, root, nameStyle(repo)),
  });
}

const FONT_THEMES = [['pixel', 'Pixel'], ['cyber', 'Cyber'], ['tech', 'Tech'], ['chip', 'Chip']];

function fontOptions(repo) {
  const chosen = repo.name_style ? repo.name_style.font : 'auto';
  const option = (value, label) => `<option value="${esc(value)}" ${chosen === value ? 'selected' : ''}>${esc(label)}</option>`;
  return option('auto', `Automatic — ${fontById(autoStyle(repo).font).family}`) + option('', 'Plain interface font')
    + FONT_THEMES.map(([theme, label]) => `<optgroup label="${label}">${FONTS.filter((f) => f.theme === theme).map((f) => option(f.id, f.family)).join('')}</optgroup>`).join('');
}

// Fills the weight list for the chosen font, keeps a valid weight and italic,
// and redraws the preview.
function syncNameStyle(repo, root, style) {
  const fontSelect = root.querySelector('#m-font');
  const weightSelect = root.querySelector('#m-weight');
  const italicSelect = root.querySelector('#m-italic');
  const font = fontById(fontSelect.value === 'auto' ? autoStyle(repo).font : fontSelect.value);
  const weights = font ? font.weights : [400, 700];
  const weight = weights.includes(style.weight) ? style.weight : weights[0];
  weightSelect.innerHTML = weights.map((w) => `<option value="${w}" ${w === weight ? 'selected' : ''}>${WEIGHT_NAMES[w] || w}</option>`).join('');
  italicSelect.value = style.italic ? '1' : '';
  italicSelect.options[1].textContent = !font || font.italics.includes(weight) ? 'Italic' : 'Slanted';
  const preview = root.querySelector('#m-preview');
  preview.innerHTML = repoName({ ...repo, name: root.querySelector('#m-name').value || repo.name, name_style: editedStyle(repo, root) || null });
  if (!fontSelect.dataset.bound) {
    fontSelect.dataset.bound = '1';
    const refresh = () => syncNameStyle(repo, root, { weight: Number(weightSelect.value), italic: italicSelect.value === '1' });
    fontSelect.addEventListener('change', () => syncNameStyle(repo, root, fontSelect.value === 'auto' ? autoStyle(repo) : { weight: Number(weightSelect.value), italic: italicSelect.value === '1' }));
    weightSelect.addEventListener('change', refresh);
    italicSelect.addEventListener('change', refresh);
    root.querySelector('#m-name').addEventListener('input', refresh);
  }
}

// The style the dialog describes, or null when it matches the automatic one,
// so a repository nobody restyled keeps following the automatic pick.
function editedStyle(repo, root) {
  const choice = root.querySelector('#m-font').value;
  const auto = autoStyle(repo);
  const style = {
    font: choice === 'auto' ? auto.font : choice,
    weight: Number(root.querySelector('#m-weight').value) || 400,
    italic: root.querySelector('#m-italic').value === '1',
  };
  return choice === 'auto' && style.weight === auto.weight && style.italic === auto.italic ? null : style;
}

function removeModal(repoId) {
  const repo = repoById(repoId);
  modal({
    title: `Remove ${repo.name}?`,
    body: `<p>DevPanel stops tracking it. The folder <span class="mono">${esc(shortPath(repo.path))}</span> and everything in it stay on disk.</p>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Remove', kind: 'danger solid', icon: 'trash',
        run: () => { bridge.send({ type: 'remove', repo: repoId }); return false; },
      },
    ],
  });
}

function wireBrowse(backdrop) {
  backdrop.querySelectorAll('[data-browse]').forEach((b) =>
    b.addEventListener('click', () => browse(backdrop.querySelector(b.dataset.browse))));
}

function exportModal() {
  const root = state.workspace.settings.clone_root || '';
  const backdrop = modal({
    title: 'Export repository list',
    body: `
      <span class="label">Save into folder</span>
      <div class="field-row"><input class="field mono" id="m-folder" value="${esc(root)}"><button class="btn" data-browse="#m-folder">${icon('folder')}Browse</button></div>
      <span class="note">Writes <span class="mono">devpanel-repos.json</span> here — each repository's remote, name, group, install command and favorite mark, but no local path. Copy it to another machine and import it there.</span>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Export', kind: 'primary', icon: 'download',
        run: (r) => {
          const folder = value(r, '#m-folder').trim();
          if (!folder) return toast('Choose a folder', 'error'), true;
          bridge.send({ type: 'export', folder });
          return false;
        },
      },
    ],
  });
  wireBrowse(backdrop);
}

function importModal() {
  const root = state.workspace.settings.clone_root || '';
  const backdrop = modal({
    title: 'Import & clone repositories',
    body: `
      <span class="label">List file is in folder</span>
      <div class="field-row"><input class="field mono" id="m-folder" placeholder="Folder holding devpanel-repos.json"><button class="btn" data-browse="#m-folder">${icon('folder')}Browse</button></div>
      <span class="label">Clone into</span>
      <div class="field-row"><input class="field mono" id="m-dest" value="${esc(root)}"><button class="btn" data-browse="#m-dest">${icon('folder')}Browse</button></div>
      <span class="note">Reads <span class="mono">devpanel-repos.json</span> from the first folder and clones every repository it lists into the second, each in its own subfolder. Ones already tracked are skipped. Output appears in Activity.</span>`,
    actions: [
      { text: 'Cancel' },
      {
        text: 'Import', kind: 'primary', icon: 'upload',
        run: (r) => {
          const folder = value(r, '#m-folder').trim();
          if (!folder) return toast('Choose the folder holding the list', 'error'), true;
          bridge.send({ type: 'import', folder, dest: value(r, '#m-dest').trim() || null });
          toast('Importing… follow it in Activity', 'info');
          return false;
        },
      },
    ],
  });
  wireBrowse(backdrop);
}

// ---- toasts ---------------------------------------------------------------------

function toast(message, level = 'info') {
  const tone = { error: 'danger', success: 'ok', info: 'info' }[level] || 'info';
  const el = document.createElement('div');
  el.className = `toast tone-${tone}`;
  el.innerHTML = `${icon(level === 'error' ? 'alert' : level === 'success' ? 'check' : 'sparkle')}<span>${esc(message)}</span>`;
  $('#toasts').appendChild(el);
  setTimeout(() => el.remove(), level === 'error' ? 9000 : 3500);
  el.addEventListener('click', () => el.remove());
}

// ---- interaction ------------------------------------------------------------------

function openDrawer(repoId) {
  state.selected = repoId;
}

function density() {
  return parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--density')) || 1;
}

function applyTheme() {
  const choice = state.workspace.settings.theme || 'system';
  const dark = choice === 'dark' || (choice === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  document.documentElement.dataset.density = state.workspace.settings.density || 'comfortable';
}
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

const ACTIONS = {
  page: (el) => goTo(el.dataset.page),
  'reset-sidebar': () => saveSettings({ sidebar_order: [] }),
  filter: (el) => { state.view = 'repos'; state.favoritesOnly = false; state.filter = el.dataset.filter; },
  layout: (el) => { state.layout = el.dataset.layout; remember('layout', state.layout); },
  select: (el) => { state.selected = state.selected === el.dataset.repo ? null : el.dataset.repo; },
  'close-drawer': () => closeDrawer(),
  'refresh-all': () => bridge.send({ type: 'refresh' }),
  'fetch-all': () => bridge.send({ type: 'fetch' }),
  'add-repo': (el) => addRepoModal(el.dataset.tab),
  open: (el) => bridge.send({ type: 'open', repo: el.dataset.repo, tool: el.dataset.tool || null }),
  'open-menu': (el) => toolMenu(el, el.dataset.repo),
  'repo-menu': (el) => repoMenu(el, el.dataset.repo),
  fix: (el) => {
    if (el.dataset.fix === 'commit-all') return commitModal(el.dataset.repo);
    bridge.send({ type: 'fix', repo: el.dataset.repo, fix: el.dataset.fix });
  },
  git: (el) => bridge.send({ type: el.dataset.op, repo: el.dataset.repo }),
  commit: (el) => commitModal(el.dataset.repo),
  install: (el) => installModal(el.dataset.repo),
  run: (el) => runModal(el.dataset.repo),
  edit: (el) => editModal(el.dataset.repo),
  remove: (el) => removeModal(el.dataset.repo),
  favorite: (el) => toggleFavorite(el.dataset.repo),
  'export-repos': () => exportModal(),
  desktop: (el) => desktop(el.dataset.desktop),
  'self-install': () => installation('install'),
  'open-in-commits': (el) => bridge.send({ type: 'open-in-commits', repo: el.dataset.repo, theme: document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light' }),
  update: () => update('stage'),
  'check-update': () => { state.update.manual = true; update('check'); },
  'import-repos': () => importModal(),
  copy: (el) => copy(el.dataset.text),
  'open-url': (el) => bridge.send({ type: 'open-url', url: el.dataset.url }),
  'toggle-job': (el) => {
    const id = Number(el.dataset.job);
    state.expandedJobs.has(id) ? state.expandedJobs.delete(id) : state.expandedJobs.add(id);
  },
  theme: (el) => {
    state.workspace.settings = { ...state.workspace.settings, theme: el.dataset.theme };
    applyTheme();
    bridge.send({ type: 'save-settings', settings: state.workspace.settings });
  },
  browse: (el) => browse(document.querySelector(el.dataset.target)),
  'add-tool': () => {
    const id = `tool-${Date.now().toString(36)}`;
    $('#tool-rows').insertAdjacentHTML('beforeend', toolRow({ id, name: '', launch: { kind: 'program', program: '', args: ['{path}'] } }));
    $('#tool-rows').lastElementChild.querySelector('input').focus();
  },
  'remove-tool': (el) => el.closest('tr').remove(),
};

// Clicks that only change local view state re-render; ones that open a menu
// or modal manage their own DOM and must not be re-rendered away.
const NO_RENDER = new Set(['open-in-commits', 'desktop', 'self-install', 'update', 'check-update', 'open-menu', 'repo-menu', 'add-repo', 'commit', 'install', 'run', 'edit', 'remove', 'favorite', 'export-repos', 'import-repos', 'browse', 'add-tool', 'remove-tool', 'copy', 'open', 'open-url', 'fix']);

document.addEventListener('click', (event) => {
  const el = event.target.closest('[data-action]');
  if (!el || el.tagName === 'SELECT') return;
  const name = el.dataset.action;
  if (!ACTIONS[name]) return;
  ACTIONS[name](el);
  if (!NO_RENDER.has(name)) render();
});

document.addEventListener('change', (event) => {
  const el = event.target;
  if (el.dataset.action === 'sort') {
    state.sort = el.value;
    remember('sort', state.sort);
    render();
  } else if (el.dataset.action === 'tool-kind') {
    const row = el.closest('tr');
    const kind = el.value;
    row.querySelector('[name=program]').disabled = kind === 'file-manager';
    row.querySelector('[name=program]').placeholder = kind === 'terminal' ? 'command to run (optional)' : kind === 'program' ? 'program or path' : '—';
    row.querySelector('[name=args]').disabled = kind !== 'program';
  }
});

document.addEventListener('submit', (event) => {
  event.preventDefault();
  const form = event.target;
  if (form.dataset.form === 'settings') {
    const data = new FormData(form);
    bridge.send({
      type: 'save-settings',
      settings: {
        ...state.workspace.settings,
        clone_root: data.get('clone_root'),
        default_tool: data.get('default_tool') || null,
        auto_fetch_minutes: Number(data.get('auto_fetch_minutes')),
        scan_depth: Math.max(1, Math.min(8, Number(data.get('scan_depth')) || 3)),
        theme: data.get('theme') || 'system',
        start_page: data.get('start_page') || 'last',
        plain_names_for_new_repos: data.get('plain_names_for_new_repos') === 'plain',
        density: data.get('density') || 'comfortable',
      },
    });
    toast('Settings saved', 'success');
  } else if (form.dataset.form === 'tools') {
    const tools = [...form.querySelectorAll('tr[data-tool]')].map((row) => {
      const field = (name) => row.querySelector(`[name=${name}]`).value.trim();
      const kind = field('kind');
      const launch = kind === 'program' ? { kind, program: field('program'), args: splitArgs(field('args')) }
        : kind === 'terminal' ? { kind, command: field('program') || null }
          : { kind };
      return { id: row.dataset.tool, name: field('name'), launch };
    });
    bridge.send({ type: 'save-tools', tools });
    toast('Tools saved', 'success');
  }
});

$('#search').addEventListener('input', (event) => {
  state.search = event.target.value;
  if (state.search && state.view !== 'repos') { state.view = 'repos'; state.filter = 'all'; }
  render();
});

document.addEventListener('keydown', (event) => {
  const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName);
  if (event.key === 'Escape') {
    if ($('#menu-root').firstElementChild) closeMenu();
    else if (modalOpen()) $('#modal-root').innerHTML = '';
    else if (typing) document.activeElement.blur();
    else if (state.selected) { closeDrawer(); render(); }
    return;
  }
  if (typing || modalOpen()) return;
  if (event.key === '/' || (event.key.toLowerCase() === 'f' && (event.ctrlKey || event.metaKey))) {
    event.preventDefault();
    $('#search').focus();
  } else if (event.key.toLowerCase() === 'n' && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    addRepoModal();
  } else if (event.key === 'F5' || (event.key.toLowerCase() === 'r' && (event.ctrlKey || event.metaKey))) {
    event.preventDefault();
    bridge.send({ type: 'refresh' });
  }
});

// Coming back to the window is when status most likely changed elsewhere.
let lastFocusRefresh = 0;
window.addEventListener('focus', () => {
  if (Date.now() - lastFocusRefresh < 5000) return;
  lastFocusRefresh = Date.now();
  bridge.send({ type: 'refresh' });
});

// Elapsed times on running jobs.
setInterval(() => { if (counts().running && state.view === 'activity' && !modalOpen()) render(); }, 1000);

hydrateIcons(document);
render();
bridge.send({ type: 'ready' });
