# Security policy

## Reporting a vulnerability

Please report security issues privately through GitHub's **"Report a vulnerability"** button on the
repository's Security tab (private vulnerability reporting). Do not open a public issue for security problems.

Include: Rekora version (`rekora --version`), OS, the command or MCP call involved, and the smallest
reproduction you can share. Do not include real secrets or private project data.

You can expect an acknowledgement within 7 days. Fixes are released as patch versions and credited in
the changelog unless you prefer otherwise.

## Scope

In scope: cross-project data leakage, path/symlink/junction escapes, secrets reaching disk despite
redaction, prompt-injected content being treated as verified, host config corruption by adapters,
crashes or hangs triggered by MCP input.

The threat model and its known gaps are documented in
[docs/architecture/SECURITY.md](docs/architecture/SECURITY.md).

## Supported versions

Only the latest released version receives fixes while Rekora is pre-1.0.
