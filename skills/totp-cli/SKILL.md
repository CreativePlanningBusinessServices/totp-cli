---
name: totp-cli
description: Use when a login flow asks for a 6-digit authenticator / TOTP / 2FA code and the account is enrolled in totp-cli, or when a task needs to enrol, list or remove TOTP accounts from the command line. Covers fetching a code safely for automation (--min-remaining), enrolling from a secret or otpauth URI, and triaging errors by exit code.
---

# totp-cli

Single-binary CLI that keeps TOTP secrets in the OS credential store (macOS Keychain,
Windows Credential Manager) and prints the current code for a named account. Built for
agents: the code is the only thing on stdout, errors go to stderr, exit codes are fixed.
Full reference: `totp-cli <subcommand> --help`.

## Pick the right command

| You need | Use |
|---|---|
| The current code for an enrolled account | `totp-cli code <name>` |
| A code that will still be valid while a form is filled and submitted | `totp-cli code <name> --min-remaining 5` |
| Code plus seconds until it rolls over | `totp-cli --json code <name>` → `{"name","code","expires_in","period"}` |
| Which accounts exist | `totp-cli list` (names only; secrets are never printed) |
| Enrol from a base32 secret | `totp-cli add <name> --secret <BASE32> [--issuer X]` |
| Enrol from a QR-code URI | `totp-cli add <name> --uri 'otpauth://totp/...'` |
| Replace a secret | `totp-cli add <name> ... --force` |
| Remove an account | `totp-cli remove <name>` |

## Recipe: a login page asks for a 2FA code

```bash
code=$(totp-cli code netsuite-prod --min-remaining 5) || exit_code=$?
```

Account names follow the convention `<service>-<environment>`, mirroring netsuite-cli's
aliases: `netsuite-prod` (production) and `netsuite-sb2` (Sandbox 2). Run `totp-cli list` to
see what is enrolled; never guess a name.

- Exit 0: `$code` is the 6-digit value; type it into the form and submit promptly.
- Exit 2 with "no account named": the account is not enrolled. **Ask the user** for the code
  (or to enrol it); never guess a name or try other accounts.
- Exit 3: the credential store refused or lost the secret. Report the stderr text to the user.

`--min-remaining 5` waits (at most a few seconds) for the next period if the current code
is about to expire, so the submitted code is never stale.

## Enrolling

Pass the secret with `--secret`, or run `totp-cli add <name>` with neither `--secret` nor
`--uri` to be prompted without echo. The secret is never echoed back or included in
`--json` output. Defaults are 6 digits, 30 seconds, SHA-1; `--uri` carries the service's
own values and explicit flags override them.

## Exit codes

| Code | Meaning | What to do |
|---|---|---|
| 0 | Success | Use stdout |
| 1 | Internal / I/O (e.g. unreadable accounts.toml) | Report stderr to the user |
| 2 | Usage: bad arguments, bad secret, unknown or duplicate account | Fix the call, or ask the user |
| 3 | Credential store unavailable, denied, or missing the secret | Report stderr; re-enrol with `add --force` if the user agrees |

## Storage

Secrets: OS credential store, service `totp-cli`, one item per account. Non-secret
parameters: `accounts.toml` in the per-user config dir (override with
`TOTP_CLI_CONFIG_DIR`). `TOTP_CLI_SECRET_STORE=file` swaps in an unencrypted
`secrets.toml` for tests and headless Linux only.

## Maintenance

`totp-cli update --check` reports a newer release; `totp-cli update` installs it and
refreshes this skill. `totp-cli skill install [--dir <path>]` rewrites the skill alone.
