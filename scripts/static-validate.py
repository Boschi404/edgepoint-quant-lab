#!/usr/bin/env python3
from pathlib import Path
import re, sys, tomllib

root = Path(__file__).resolve().parents[1]
errors: list[str] = []

# Cargo TOML parse + duplicate dependency keys in plain text.
for p in [root / 'Cargo.toml', *root.glob('crates/*/Cargo.toml')]:
    try:
        tomllib.loads(p.read_text())
    except Exception as exc:
        errors.append(f'{p}: invalid TOML: {exc}')
    in_deps = False
    seen: set[str] = set()
    for line in p.read_text().splitlines():
        stripped = line.strip()
        if stripped == '[dependencies]':
            in_deps = True
            continue
        if stripped.startswith('['):
            in_deps = False
        if in_deps and '=' in stripped and not stripped.startswith('#'):
            key = stripped.split('=', 1)[0].strip()
            if key in seen:
                errors.append(f'{p}: duplicate dependency key {key}')
            seen.add(key)

# Runtime Rust code should not use unwrap()/expect(). Tests are allowed to panic but still avoid direct calls.
for p in root.glob('crates/**/*.rs'):
    text = p.read_text()
    if re.search(r'\.unwrap\s*\(', text):
        errors.append(f'{p}: direct unwrap() call found')
    if re.search(r'\.expect\s*\(', text):
        errors.append(f'{p}: direct expect() call found')

# Required high-value files.
required = [
    'Dockerfile', 'Dockerfile.production', 'docker-compose.yml',
    'crates/qs-api/src/lib.rs', 'crates/qs-app/src/runtime_launcher.rs',
    'configs/datasets.toml', 'data/sample_ohlcv.csv',
    'scripts/e2e-smoke.sh', 'docs/runbooks/END_TO_END_RUN.md',
]
for rel in required:
    if not (root / rel).exists():
        errors.append(f'missing required file {rel}')

if errors:
    for e in errors:
        print(f'[error] {e}', file=sys.stderr)
    sys.exit(1)
print('[ok] static validation passed')
