#!/usr/bin/env python3
"""Build the standalone relay deployment archive without local secrets or caches."""
import gzip
import hashlib
import io
from pathlib import Path
import tarfile
import tomllib

root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
output = root / 'target' / f'draimhorizons-relay-{version}.tar.gz'
output.parent.mkdir(exist_ok=True)
files = [p for p in (root / 'relay').rglob('*') if p.is_file()
         and not any(part in ('__pycache__', '.venv', '.git') for part in p.parts)
         and (not p.name.startswith('.env') or p.name == '.env.example')
         and p.suffix not in ('.pyc', '.log')]
with output.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as compressed:
    with tarfile.open(fileobj=compressed, mode='w') as archive:
        for path in sorted(files):
            data = path.read_bytes()
            info = tarfile.TarInfo(str(path.relative_to(root)))
            info.size = len(data)
            info.mode = 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(data))
digest = hashlib.sha256(output.read_bytes()).hexdigest()
output.with_name(output.name + '.sha256').write_text(f'{digest}  {output.name}\n')
print(f'{output.name}: {len(files)} files, {output.stat().st_size} bytes, SHA256 {digest}')
