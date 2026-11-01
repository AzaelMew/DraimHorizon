#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
command -v codex >/dev/null
command -v python3 >/dev/null
command -v node >/dev/null
command -v npm >/dev/null
readarray -t ecc_lock < <(python3 - "$repo_root/.ecc/upstream-lock.json" <<'PYLOCK'
import json, sys
lock = json.load(open(sys.argv[1]))['ecc']
for key in ('repository', 'revision', 'version'):
    print(lock[key])
PYLOCK
)
ecc_source="$repo_root/.local/ecc"
mkdir -p "$repo_root/.local"
if [[ ! -d "$ecc_source/.git" ]]; then
    git init "$ecc_source"
    git -C "$ecc_source" remote add origin "${ecc_lock[0]}"
fi
git -C "$ecc_source" fetch --depth 1 origin "${ecc_lock[1]}"
git -C "$ecc_source" checkout --detach "${ecc_lock[1]}"
codex plugin list --json > "$repo_root/.local/codex-plugins.json"
ecc_state="$(python3 - "$repo_root/.local/codex-plugins.json" "${ecc_lock[2]}" <<'PYSTATE'
import json, sys
plugins = [p for p in json.load(open(sys.argv[1]))['installed'] if p.get('pluginId') == 'ecc@ecc']
if plugins and any(p.get('version') != sys.argv[2] for p in plugins):
    raise SystemExit('Different ECC version already installed; inspect the marketplace before updating.')
print('ready' if any(p.get('enabled') for p in plugins) else 'missing')
PYSTATE
)"
if [[ "$ecc_state" != ready ]]; then
    codex plugin marketplace add "$ecc_source"
    codex plugin add ecc@ecc
fi
codex plugin list --json > "$repo_root/.local/codex-plugins.json"
python3 - "$repo_root/.local/codex-plugins.json" "${ecc_lock[2]}" <<'PYVERIFY'
import json, sys
plugins = json.load(open(sys.argv[1]))['installed']
assert any(p.get('pluginId') == 'ecc@ecc' and p.get('enabled') and p.get('version') == sys.argv[2] for p in plugins), 'Expected ECC version is not enabled; inspect existing ecc marketplace before changing it.'
PYVERIFY
node "$ecc_source/scripts/codex/check-plugin-cache.js" --version "${ecc_lock[2]}"
ecc_cache="$(node -p 'require("path").join(process.env.CODEX_HOME || require("path").join(require("os").homedir(), ".codex"), "plugins/cache/ecc/ecc", process.argv[1])' "${ecc_lock[2]}")"
(
    cd -- "$ecc_cache"
    test -f package.json
    test -f package-lock.json
    npm ci --omit=dev --ignore-scripts --no-audit --no-fund --no-bin-links
    node -e "require('js-yaml'); require('@iarna/toml'); require('ajv'); require('sql.js'); require('zod')"
)
printf '%s\n' 'ECC installed. Restart Codex to load skills and project roles; review optional hooks in /hooks.'
