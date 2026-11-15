# Everyday tasks. `just` lists them; each one is a plain command you can also run yourself.
# Needs the prerequisites in docs/development.md and PowerShell 7 for the helper scripts.

set windows-shell := ["pwsh", "-NoProfile", "-Command"]

# List the recipes.
default:
    @just --list

# Check out the vendored engine; the first build needs it.
submodules:
    git submodule update --init --recursive

# Build a debug binary.
build:
    cargo build -p devpanel

# Build the optimised binary.
release:
    cargo build --release -p devpanel

# Run DevPanel from the working tree, optionally adding a folder.
run *args:
    cargo run --release -p devpanel -- {{args}}

# Run every test.
test:
    cargo test --workspace

# Check the code with clippy, warnings as errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Build the browser preview of the page into .artifacts/preview.
preview:
    pwsh scripts/preview.ps1

# Screenshot a view at the given widths; a repository id opens its details window.
screenshot view="repos" theme="light" width="1280,900" select="":
    pwsh scripts/screenshot.ps1 -View {{view}} -Theme {{theme}} -Width {{width}} -Select "{{select}}"

# Assemble dist/app in the install layout.
dist:
    pwsh scripts/dist.ps1

# Install DevPanel for the current user, or update an existing install.
install:
    pwsh scripts/install.ps1
