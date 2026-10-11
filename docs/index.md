# Documentation

The documentation describes intent; where it and the code disagree, the code is wrong. Each page sits at one level, so an ordinary refactoring should not require editing any of them.

| Page | Read it to learn |
| --- | --- |
| [architecture.md](architecture.md) | The layers, what may depend on what, and how an action travels from a click to `git` and back |
| [design/domain.md](design/domain.md) | How a repository's health is judged, which fixes apply, and how install recipes are found |
| [design/protocol.md](design/protocol.md) | The JSON messages between the page and the host |
| [installation.md](installation.md) | How DevPanel is installed and updated: launcher, version folders, scripts |
| [development.md](development.md) | Building, testing, previewing the UI, and platform notes |
| [adr/](adr/) | Why the larger decisions were made |

Behaviour still to confirm with the owner is listed in [VERIFY.md](../VERIFY.md).
