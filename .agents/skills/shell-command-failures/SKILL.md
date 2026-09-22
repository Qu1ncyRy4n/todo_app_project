---
name: shell-command-failures
description: Handle shell command failures explicitly and preserve meaningful diagnostics. Use when writing or reviewing shell commands, scripts, cleanup, probes, or command orchestration.
---
# Handle Shell Command Failures

## Make Failure Handling Visible

Do not hide a command failure with `|| true`. When a command may fail without
failing the operation, branch explicitly and preserve the exit status or useful
diagnostic output.

## Treat Cleanup And Probes Deliberately

Cleanup, probing, and optional diagnostics may be non-fatal, but their handling
must stay visible. Report a cleanup failure when it affects user data, generated
output, or confidence in validation.

## Verify Failure Paths

For shell changes, run the narrowest relevant syntax, lint, or behavioral check
available in the target repository. Test expected failures explicitly; do not
let setup failure or unavailable external services create accidental passing
tests.
