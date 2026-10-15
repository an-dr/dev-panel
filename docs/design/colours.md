# Colours

DevPanel's palette and the rule for when something gets colour. The values live as tokens at the top of [style.css](../../apps/devpanel/web/style.css); this page says what they mean.

DevPanel hands the same palette to Commits as a theme file in Commits' format, [commits-theme.json](../../apps/devpanel/assets/commits-theme.json), written beside the workspace file at startup and passed with every "Open in Commits". It is generated from this page by hand: change a colour here, then there. A test holds its urgency colours to the page's tokens, and Commits' own [theme-file format](https://github.com/an-dr/commits/blob/main/docs/themes.md) decides its keys. The light theme's selection is the aqua `#c9f0ea`; borders and the dark selection are the page's `color-mix` values worked out, since a theme file takes plain colours.

## Brand

| Colour | Value | Used for |
| --- | --- | --- |
| Navy | `#191b39` | Light theme text, outlines and primary-button outlines; the logo's fill |
| Cyan | `#9afff5` | Dark theme accents, outlines and primary buttons; the logo's line |

Cyan is the accent. It marks what you can act on and where you are, and it never means urgency.

## Dark cockpit

Whatever is normal stays neutral, and colour only means something needs a look. A clean repository, a finished job, a zero count and a total all render in text greys, with no tint and no stripe. Colour appears only when a state is out of the ordinary, and its hue says how urgent:

| Urgency | Hue | Dark | Light text | Light solid fill | Verdicts |
| --- | --- | --- | --- | --- | --- |
| normal | none | text greys | text greys | — | clean |
| advisory | yellow-orange | `#ffb84a` | `#b86e00` | `#ffb84a`, navy text | behind, unpushed, unpublished |
| caution | red-orange | `#ff7b4a` | `#d2501c` | `#d2501c`, white text | in progress, diverged, uncommitted, detached HEAD |
| warning | pink | `#ff5c9d` | `#e0287a` | `#e0287a`, white text | missing, not a repository, conflicts |

The verdicts themselves are defined in [domain.md](domain.md#verdicts). Green (`#5effa8` dark, `#008a4e` light) is kept for short-lived success messages and is never a resting state.

Soft backgrounds are each colour mixed into the surface: about 14% in the dark theme and 10% in the light theme.

## Surfaces and outlines

| | Dark | Light |
| --- | --- | --- |
| Page | `#12131c` | `#f4f5fa` |
| Panels and cards | `#1b1c2a` | `#ffffff` |
| Outlines, 2px | cyan, 42% into the panel colour | navy, 48% into white |
| Primary buttons | flat, cyan text and outline on a 14% cyan fill | flat, navy text and outline on aqua `#c9f0ea` |

Containers carry a 2px outline. Dividers inside a container stay a faint 1px line, so the containers read as the shapes.
