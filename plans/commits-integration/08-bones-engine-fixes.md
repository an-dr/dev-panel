# 08 — bones: engine fixes for panel serving and Wayland

- **Repository:** bones (`bones-module-web` and the kernel's platform)
- **Depends on:** nothing
- **Needed by:** optional adoptions in 10 (A) and 16 (B)
- **Priority:** optional. Both apps already work around these.

Two independent fixes. Each is its own change.

## A. Serve HTML panels off the window thread

**Problem (Commits BUG-005, ADR-012 § Rejected alternatives).** `PanelSource::Html` answers its custom-protocol requests on the window-owning thread. When that thread is busy, for example loading a WASM component at startup, the panel stays blank for the whole wait. A `data:` URL renders, but it leaves the page at an opaque origin, where wry panics on any `window.ipc` call. So Commits runs its own local `TcpListener` page server (`apps/commits/host/src/page.rs`, `splash.rs`).

**Build.** Make the HTML panel source answer requests without needing the window thread to be free. Precompute the response when the panel opens and serve it from a handler that does no work on that thread. Keep the page's origin non-opaque, so `window.ipc` works. Inside `bones-module-web` only.

**Acceptance.** A test or example blocks the frame thread for 3 s right after opening an HTML panel, and the panel still paints. `window.ipc.postMessage` works from that page. Commits can retire its page server (brief 10).

## B. Web panels on sessions with `GDK_BACKEND=wayland`

**Problem.** On desktops that export `GDK_BACKEND=wayland`, such as COSMIC on Pop!_OS 24.04, `gtk::init` has failed with "Failed to initialize GTK". DevPanel works around it in `apps/devpanel/src/main.rs` (`prefer_x11`): it sets `GDK_BACKEND=x11` and `SDL_VIDEO_DRIVER=x11` in its own environment before the engine starts, and restores the user's values for every program it launches. Engine 1.1.1 changed `bones-module-web` to "select X11 through GDK APIs without changing the process environment" (bones `CHANGELOG.md`), so the GTK half may already be fixed. SDL may still pick Wayland.

**First, measure.** On a session with `GDK_BACKEND=wayland` exported, run the bones web example with no app-side workaround, and record the result in the change description.

**Build.** Whatever still fails, fix inside bones **without changing the process environment**: select GTK's backend through GDK APIs, and SDL's video driver through `SDL_SetHint(SDL_HINT_VIDEO_DRIVER, "x11")` before SDL initialises, in the kernel's platform. Because the environment is never touched, every child process inherits the user's values with no restore step. That is why brief 05 needs no environment helper.

**Acceptance.** On a session with `GDK_BACKEND=wayland` exported, the bones web example opens its panel with no app-side workaround, and a child process started from it sees the user's original `GDK_BACKEND`. A test asserts that building the engine leaves `std::env::var("GDK_BACKEND")` and `SDL_VIDEO_DRIVER` unchanged.

## Out of scope

- Native Wayland support for the web panel. X11 through Xwayland remains the path.
