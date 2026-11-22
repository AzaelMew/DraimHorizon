# Roblox game guide

Use for the owner's `roblox-game` repository (https://github.com/DraimCiDo/roblox-game).
Facts below were read from the repository on 2026-11-22; re-check against the current
files. Project commands are not approved by being listed here.

## Stack and layout

- Luau with `--!strict`; Rojo project `default.project.json`; tools pinned in `rokit.toml`:
  Rojo, Selene, StyLua, Wally, Lune. Packages are generated; never edit `Packages/`.
- `src/server` (Services, entry `Main.server.luau`), `src/client` (Controllers),
  `src/shared` (Loader, Remotes, Types, Config, Util). Modules load via `Loader`
  `Init` then `Start`. Shared code is visible to exploiters: no secrets there.
- Project docs: `docs/GDD.md`, `ARCHITECTURE.md`, `ECONOMY.md`, `WORKFLOW.md`, ADRs
  in `docs/decisions`. Project skills in `.claude/skills/` (new-module, new-remote,
  datastore-change, pre-pr-check, studio-mcp); Codex reads them manually.

## Commands (each needs plan approval before running)

`rokit install`, `wally install`, `stylua src`, `stylua --check src`, `selene src`,
`rojo build default.project.json -o build/game.rbxl`, `./scripts/check.sh` (or
`check.ps1`) as the CI-equivalent. Never edit scripts in Studio; `src/` is the source.

## Behavior

- Server is authoritative; validate every Remote argument; saves only through
  `PlayerDataService`. Never change `DataStoreName`; schema changes need the
  datastore-change procedure and a plan-listed approval (saved-data change).
- Do not guess game design; ask when `docs/GDD.md` and the task are silent.
  Do not invent Roblox API; verify against official docs.
- New Wally dependencies need approval. No Toolbox code without approval.
- One branch and one draft PR per body of work. Never use `--no-verify`, force push,
  or `git reset --hard`; do not commit `Packages/`, `*.rbxl`, `sourcemap.json`.
- Studio via MCP: playtests and script inspection only after plan approval; only one
  agent uses Studio at a time. Say plainly when Studio checks were not possible.

## Playbook overrides for this project

The playbook wins on process: English for code, comments, commits, and PRs; PR body
uses Problem/Changes/Verification/Risks (put Studio reproduction steps in Verification);
checks run only if listed in the approved plan (including `scripts/check`); no
AI attribution in commits; the project's `ai:made` label and the PR template's AI
checkbox are not applied unless the owner asks. `.claude/settings.json` pre-allows
`rojo build`, `stylua`, `selene`, `./scripts/check.sh`: treat that as app permission,
not as plan approval.
