# 14 — Commits: theme flags and theme files

- **Repository:** commits
- **Depends on:** 10, and 12 for the urgency tokens
- **Needed by:** 18 (DevPanel passes its mode and its theme file)
- **Contracts:** extends contract 1 with `--theme` and `--theme-file`, and creates contract 5 from the draft in [00-contracts.md](00-contracts.md#5-theme-files)
- **Priority:** recommended

## Why

Opened from DevPanel, Commits should look like part of the same product: the same light or dark mode, and colours from the same family. Today Commits has its own appearance settings: `app.mode` is `system`, `light` or `dark`, with Paper and Graphite presets (`docs/settings.md`, `apps/commits/web/src/themes.ts`).

Commits does not ship a DevPanel preset. It would make Commits depend on DevPanel's brand, and every palette change in DevPanel would need a Commits release. Instead a caller passes a **theme file** in a format Commits owns, for one session. DevPanel ships its own file (brief 18). Any other caller, or a user, can do the same.

## Architecture

| Piece | Where |
| --- | --- |
| Parsing the flags | `launch_request.rs`: new fields `theme: Option<ThemeMode>` and `theme_file: Option<PathBuf>`, resolved against the launch's working directory |
| Reading the file | the host reads it once, on startup or handoff, with a size limit of 256 KB, and passes the parsed JSON to the guest with the request |
| Validating it | pure TypeScript in `apps/commits/web/src/themes.ts`: `parseThemeFile(json): ThemeFile \| Error` |
| Applying it | `resolveAppearance` takes an optional session override `{ mode?, file? }` |

The session override is never written to `settings.json`.

## What to build

1. **`--theme light|dark`.** Sets the mode for this session only. Any other value is ignored.
2. **`--theme-file <file>`.** Loads the palette for this session only, for both modes. It is validated as contract 5 says. An invalid file is ignored, with a one-line non-modal notice naming the file and the first problem.
3. **Both apply on a handoff** as at startup, through brief 10's launch request. A later handoff without the flags keeps the session's current override; one with the flags replaces it. Settings changes made in the window during the session apply on top of the mode, as they do today.
4. **The contract.** Create `docs/themes.md` from the draft: the keys, the value forms, the rules for invalid files, and an example file. Add both flags to `docs/command-line.md`.
5. **Graph colours.** Lane colours stay Commits' own: they tell branches apart, not urgency. A theme file does not set them.

## Acceptance

- `commits <repo> --theme dark` opens dark while settings say light, and `settings.json` is byte-identical afterwards.
- `commits <repo> --theme-file <valid file>` renders with that file's colours, including the health line's urgency colours from brief 12.
- An invalid file, such as a missing key, a bad colour or bad JSON, opens with Commits' own theme and the notice.
- Both work as a handoff to a running window.
- Tests cover `parseThemeFile` for valid, missing-key, bad-value and unknown-key files; unknown keys are ignored.

## Out of scope

- Theme files chosen in Settings, or a themes folder. A caller passes the file each launch.
- Changing DevPanel to read Commits' themes.
