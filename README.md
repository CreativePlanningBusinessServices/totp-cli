# totp-cli

Generate TOTP (RFC 6238) codes from secrets kept in the OS credential store — macOS Keychain or
Windows Credential Manager. Built so an agent can answer a 2FA prompt itself: the code is the only
thing on stdout, errors go to stderr, exit codes are fixed.

```bash
totp-cli add netsuite --issuer NetSuite      # prompts for the base32 secret without echo
totp-cli code netsuite                       # → 287082
totp-cli code netsuite --min-remaining 5     # waits for the next code if this one is about to expire
totp-cli list                                # → netsuite
totp-cli remove netsuite
```

## Install

Download the archive for your platform from the
[Releases page](https://github.com/CreativePlanningBusinessServices/totp-cli/releases), unzip it,
and put `totp-cli` (or `totp-cli.exe`) on your `PATH`:

```bash
gh release download -R CreativePlanningBusinessServices/totp-cli --pattern "*aarch64-apple-darwin*"
unzip -o totp-cli-*.zip && install -m 0755 totp-cli "$HOME/.local/bin/"
```

Releases are published for `aarch64-apple-darwin`, `x86_64-apple-darwin` and
`x86_64-pc-windows-msvc`. Or build from source:

```bash
cargo install --git https://github.com/CreativePlanningBusinessServices/totp-cli
```

## Usage

### `add` — enrol an account

```bash
totp-cli add <name> [--secret <BASE32> | --uri 'otpauth://totp/...']
                    [--digits 6|7|8] [--period <secs>] [--algorithm sha1|sha256|sha512]
                    [--issuer <text>] [--force]
```

- `<name>` is your local alias: 1–64 characters from `A-Z a-z 0-9 . _ -`.
- With neither `--secret` nor `--uri`, the secret is read from the terminal without echo so it stays
  out of shell history.
- `--uri` takes the `otpauth://totp/...` URI behind a QR code and pulls `secret`, `issuer`, `digits`,
  `period` and `algorithm` from it. Explicit flags override the URI's values.
- Base32 input is normalised: case, spaces, dashes and `=` padding do not matter.
- Defaults: 6 digits, 30-second period, SHA-1.
- An existing name is refused unless you pass `--force`.

```
$ totp-cli add netsuite --secret GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ --issuer NetSuite
added netsuite
$ totp-cli --json add ns --uri 'otpauth://totp/NetSuite:jon?secret=GEZDGNBVGY3TQOJQ&digits=8'
{"algorithm":"sha1","digits":8,"issuer":"NetSuite","name":"ns","period":30}
```

### `code` — print the current code

```bash
totp-cli code <name> [--min-remaining <secs>]
```

```
$ totp-cli code netsuite
287082
$ totp-cli --json code netsuite
{"code":"287082","expires_in":17,"name":"netsuite","period":30}
```

`--min-remaining N` makes `code` wait for the next period when the current code expires in fewer
than `N` seconds, so an automated login never submits a code that rolls over mid-form. `N` must be
less than the account's period.

### `list` and `remove`

```
$ totp-cli list
netsuite
$ totp-cli --json list
[{"algorithm":"sha1","digits":6,"issuer":"NetSuite","name":"netsuite","period":30}]
$ totp-cli remove netsuite
removed netsuite
```

`list` never prints secrets; it reads only the index file.

## Automation notes

- `--json` is a global flag. Successful output is one JSON value on stdout; errors become
  `{"error":"<message>","exit_code":N}` on stderr. Without `--json`, errors are `error: <message>`.
- stdout carries only the result, so `code=$(totp-cli code netsuite)` is safe.

| Exit code | Meaning | Examples |
|---|---|---|
| 0 | Success | |
| 1 | Internal / I/O | Unreadable `accounts.toml`, update download failed |
| 2 | Usage | Bad name, bad base32, bad URI, unknown account, duplicate without `--force`, `--min-remaining` ≥ period |
| 3 | Secret store | Keychain / Credential Manager unavailable or denied; secret missing for an indexed account |

## Storage

Only the base32 secret goes into the OS credential store, one item per account under the service
name `totp-cli`:

| Platform | Backend |
|---|---|
| macOS | Keychain. The first read by a newly built binary may show one keychain dialog; **Always Allow** makes later reads unattended. |
| Windows | Credential Manager (generic credential). |
| Linux | No credential-store backend; set `TOTP_CLI_SECRET_STORE=file` (see below). |

Everything else — issuer, digits, period, algorithm — lives in `accounts.toml` in the per-user
config directory:

| Platform | Path |
|---|---|
| macOS | `~/Library/Application Support/com.CreativePlanning.totp-cli/accounts.toml` |
| Windows | `%APPDATA%\CreativePlanning\totp-cli\config\accounts.toml` |
| Linux | `~/.config/totp-cli/accounts.toml` |

`TOTP_CLI_CONFIG_DIR` overrides the directory. The file is written atomically and, on Unix, with
mode 0600.

`TOTP_CLI_SECRET_STORE=file` swaps the credential store for `secrets.toml` in the same directory.
That file is **not encrypted at rest**; it exists for the test suite, CI, and headless Linux hosts.

## Updating

```bash
totp-cli update --check   # report whether a newer release exists; installs nothing
totp-cli update           # download and install the latest release in place
```

After swapping the binary, `update` also refreshes the bundled agent skill (below). Set
`GITHUB_TOKEN` if you hit GitHub's anonymous rate limit.

## Agent skill

The binary embeds `skills/totp-cli/SKILL.md`, a usage guide for Claude Code and similar agents.

```bash
totp-cli skill install              # writes ~/.claude/skills/totp-cli/SKILL.md ($CLAUDE_CONFIG_DIR if set)
totp-cli skill install --dir <path> # anywhere else
```

A symlinked or already-identical copy is left alone.

## Development

```bash
cargo test                                   # unit + integration tests; never touches a real keychain
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

Integration tests run the binary with `TOTP_CLI_CONFIG_DIR` pointed at a temp dir and
`TOTP_CLI_SECRET_STORE=file`. CI runs the suite on macOS, Windows and Linux; pushing a `v*` tag
builds and publishes the release archives.
