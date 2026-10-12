# Agent instructions

Read [README.md](README.md) and [docs/index.md](docs/index.md) first; [docs/development.md](docs/development.md) covers building, testing and the UI preview.

## UI changes

After every iteration that changes the page (`apps/devpanel/web/`), take screenshots and show them to the user before reporting the iteration done:

```sh
pwsh scripts/screenshot.ps1 -View <overview|repos|activity|settings> -Theme <light|dark> -Width 1280,900
```

Capture each view the change touches, at both widths — below 980 px the sidebar collapses to icons — and in the theme the change affects, or both when colours changed. Look at every image yourself before showing it; a screenshot that shows a defect means the iteration is not done.

## Icons

Icons come from [Lucide](https://lucide.dev) and are inlined in `apps/devpanel/web/icons.js`, because the page cannot load anything from the network. Add an icon by copying its paths from the same `lucide-static` version named in that file; do not draw new ones.
