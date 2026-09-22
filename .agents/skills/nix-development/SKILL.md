---
name: nix-development
description: Inspect and change Nix project or system configuration safely, preferring non-activating checks and explicit machine-level approval. Use when changing Nix files, flakes, development shells, or host configuration.
---
# Develop Nix Changes Safely

## Choose The Right Boundary

Use Nix when a repository needs reproducible development tools, build inputs,
or cross-platform environment definition. Prefer a repository-local flake and
`nix develop` for project tooling.

Do not introduce Nix, change the supported development interface, or require
Nix for ordinary runtime use without a repository decision. Keep application
commands usable through the repository's documented interface; Nix may provide
that interface, but should not become an unexplained prerequisite.

Treat global, host, or system configuration as high-impact work. Explain the
affected boundary, alternatives, expected result, and validation before making
the change. Ask before changing an active machine.

Read the repository's host layout, module layout, overlays, and feature flags
before structural edits. Keep shared behavior behind existing host boundaries
and host-specific behavior in the appropriate host configuration.

## Protect Active Machines

Ask before an activating system command. Do not change hardware configuration,
bootloader, storage, firewall, VPN, DNS, networking, shell startup behavior, or
secrets unless the request explicitly includes that area.

## Work With Flakes Deliberately

For a flake repository, inspect `flake.nix`, `flake.lock`, and the affected
module path before editing. Do not update `flake.lock` unless the task includes
dependency updates or a required check/build refreshes it. Summarize lockfile
changes when they occur.

Prefer `nix develop` or the repository's documented development shell before
debugging missing tools.

## Verify Without Activating

Prefer non-activating checks and builds before any activation. Run `nix flake
check` when supported, then build the affected host, package, or home
configuration when practical. State explicitly whether any activation was not
run.

## Keep Secrets Out Of Configuration

Do not commit secret values, private keys, tokens, or local credentials in Nix
files, lockfiles, fixtures, or generated output. Use the repository's approved
secret mechanism. If none exists, stop before inventing one that affects an
active machine or external service.
