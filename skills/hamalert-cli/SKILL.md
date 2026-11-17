---
name: hamalert-cli
description: Use the hamalert-cli command-line tool to manage HamAlert.org triggers, credentials, backups, restores, imports, bulk deletion, and trigger profiles. Use when a user asks an agent to configure HamAlert credentials, add callsign alerts, pause or resume triggers, import Ham2K PoLo or local callsign files, back up or restore triggers, inspect trigger/profile status, switch profiles, manage permanent triggers, or plan safe hamalert-cli operations.
---

# HamAlert CLI

## Overview

Use this skill to operate `hamalert-cli` safely on behalf of a user. The CLI talks to the live HamAlert.org account configured on the machine, so treat credential handling and destructive trigger operations carefully.

## Start Here

1. Determine whether the user wants a read-only check, a dry-run plan, or a live modification.
2. Run `scripts/local-check.sh` to identify how to invoke the CLI in the current environment.
3. Read `references/safety.md` before any command that can modify HamAlert state.
4. Read the task-specific reference:
   - Command syntax: `references/commands.md`
   - Common workflows: `references/workflows.md`
   - File and JSON formats: `references/data-formats.md`
   - Failures and environment issues: `references/troubleshooting.md`

## Invocation

Prefer an installed binary:

```bash
hamalert-cli <command>
```

When working from this repository and no binary is installed, use:

```bash
cargo run -- <command>
```

For generated help, run `scripts/help-snapshot.sh`.

## Operating Rules

- Never print passwords, ask for passwords in chat, or write secrets into files unless the user explicitly instructs a local config fallback.
- Prefer `auth login --password-stdin` or `auth login --password-env`; avoid `--password` except when the user explicitly accepts process-history exposure.
- Ask for explicit confirmation before live modifications unless the user already gave clear approval.
- Prefer dry-run flows before live changes. Note that many dry-runs still log in and fetch live data.
- Back up triggers before destructive operations when the command does not already create an auto-backup.
- Do not assume README examples are complete. Check `hamalert-cli <command> --help` or `scripts/help-snapshot.sh` when exact syntax matters.

## Quick Tasks

Read `references/workflows.md` for full recipes. Common starting points:

```bash
hamalert-cli auth status
hamalert-cli backup
hamalert-cli add-trigger --callsign W1AW --comment "W1AW spotted" --actions app
hamalert-cli import-file --file callsigns.txt --comment "Local imports" --actions app --dry-run
hamalert-cli profile status
hamalert-cli profile switch portable
```
