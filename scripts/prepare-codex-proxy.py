#!/usr/bin/env python3
"""Prepare the pinned, verified CPA resource for current platform. No credentials used."""
import hashlib
import io
import json
from pathlib import Path
import platform
import sys
import tarfile
import urllib.request
import zipfile

root = Path(__file__).resolve().parents[1] / 'src-tauri/resources/codex'
manifest = json.loads((root / 'manifest.json').read_text())

# Detect current platform
system = platform.system()
machine = platform.machine().lower()

# Map Python platform names to manifest keys
platform_map = {
    ('Darwin', 'arm64'): 'darwin-aarch64',
    ('Darwin', 'x86_64'): 'darwin-x86_64',
    ('Windows', 'amd64'): 'windows-x86_64',
    ('Windows', 'x86_64'): 'windows-x86_64',
    ('Windows', 'arm64'): 'windows-aarch64',
    ('Linux', 'x86_64'): 'linux-x86_64',
    ('Linux', 'aarch64'): 'linux-aarch64',
    ('Linux', 'arm64'): 'linux-aarch64',
}

platform_key = platform_map.get((system, machine))
if not platform_key:
    print(f'Warning: Platform {system} {machine} not supported for Codex sidecar')
    print('Codex/ChatGPT login will not be available')
    sys.exit(0)

if platform_key not in manifest['platforms']:
    print(f'Warning: Platform {platform_key} not in manifest')
    print('Codex/ChatGPT login will not be available')
    sys.exit(0)

config = manifest['platforms'][platform_key]
binary_name = config['binaryName']
binary = root / binary_name

# Check if already verified
if binary.exists():
    current_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    if config['binarySha256'] and current_hash == config['binarySha256']:
        print(f'CLIProxyAPI 8.0.27 ({platform_key}) verified')
        sys.exit(0)
    elif not config['binarySha256']:
        # Binary SHA256 not set in manifest, skip verification but warn
        print(f'CLIProxyAPI 8.0.27 ({platform_key}) present (SHA256: {current_hash})')
        print('Warning: Binary SHA256 not verified (not set in manifest)')
        sys.exit(0)

print(f'Downloading CLIProxyAPI 8.0.27 for {platform_key}...')
with urllib.request.urlopen(config['url'], timeout=120) as response:
    archive = response.read(40 * 1024 * 1024)

# Verify archive checksum
archive_hash = hashlib.sha256(archive).hexdigest()
if archive_hash != config['archiveSha256']:
    raise SystemExit(f'Archive checksum mismatch: expected {config["archiveSha256"]}, got {archive_hash}')

# Extract binary based on archive type
if config['url'].endswith('.zip'):
    # Windows uses zip
    with zipfile.ZipFile(io.BytesIO(archive)) as zf:
        if binary_name not in zf.namelist():
            raise SystemExit(f'Binary {binary_name} not found in archive')
        payload = zf.read(binary_name)
else:
    # macOS and Linux use tar.gz
    with tarfile.open(fileobj=io.BytesIO(archive), mode='r:gz') as tar:
        member = tar.getmember(binary_name)
        if not member.isfile():
            raise SystemExit(f'Invalid archive member: {binary_name}')
        payload = tar.extractfile(member).read()

# Verify binary checksum if set
payload_hash = hashlib.sha256(payload).hexdigest()
if config['binarySha256']:
    if payload_hash != config['binarySha256']:
        raise SystemExit(f'Binary checksum mismatch: expected {config["binarySha256"]}, got {payload_hash}')
else:
    print(f'Warning: Binary SHA256 not verified (not set in manifest): {payload_hash}')

# Write binary
staged = root / f'{binary_name}.download'
staged.write_bytes(payload)
if system != 'Windows':
    staged.chmod(0o755)
staged.replace(binary)

print(f'CLIProxyAPI 8.0.27 ({platform_key}) ready')
