# The workspace file, for other programs

DevPanel keeps its repositories, tools and settings in one JSON document. Other programs may read part of it. Commits does, to list DevPanel's repositories in its repository selector. This page is the contract for those readers: what they may rely on, and what DevPanel promises not to break.

## Location

| Platform | Path |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/devpanel/workspace.json`, normally `~/.config/devpanel/workspace.json` |
| Windows | `%APPDATA%\devpanel\workspace.json` |
| macOS | `~/Library/Application Support/devpanel/workspace.json` |

`DEVPANEL_CONFIG` overrides the path, for DevPanel and for readers alike.

## What a reader may use

```json
{
  "repos": [
    { "id": "…", "name": "bones", "path": "C:\\Code\\bones", "group": "engine", "favorite": true }
  ],
  "settings": { "theme": "system" }
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `repos[].id` | string | Stable across renames and moves |
| `repos[].name` | string | Display name |
| `repos[].path` | string | Absolute; may start with `~` for the home folder; may no longer exist |
| `repos[].group` | string or absent | Free-text group |
| `repos[].favorite` | bool or absent | Absent means false |
| `settings.theme` | `system`, `light` or `dark` | DevPanel's appearance mode |

## Rules for readers

- **Read only.** Never write the file. DevPanel rewrites it after every change and would overwrite edits.
- **Ignore everything else.** Other fields exist and change without notice.
- **A missing or malformed file is normal.** It means DevPanel is not installed, or is mid-save. Keep the last good read, or show nothing, never an error.

## DevPanel's promise

The fields in the table keep their names, types and meanings. Renaming or removing one, or changing what it means, is a breaking change: it is announced in the release notes, and Commits is updated in step. Adding fields is not breaking.
