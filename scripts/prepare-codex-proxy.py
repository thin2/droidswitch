#!/usr/bin/env python3
"""Prepare the pinned, verified macOS arm64 CPA resource. No credentials used."""
import hashlib
import io
import json
from pathlib import Path
import platform
import tarfile
import urllib.request

root = Path(__file__).resolve().parents[1] / 'src-tauri/resources/codex'
manifest = json.loads((root / 'manifest.json').read_text())
if (platform.system(), platform.machine()) != ('Darwin', 'arm64'):
    raise SystemExit('Codex sidecar currently supports macOS arm64 only')
binary = root / 'cli-proxy-api'
if binary.exists() and hashlib.sha256(binary.read_bytes()).hexdigest() == manifest['binarySha256']:
    print('CLIProxyAPI 8.0.27 verified')
    raise SystemExit(0)
with urllib.request.urlopen(manifest['url'], timeout=120) as response:
    archive = response.read(40 * 1024 * 1024)
if hashlib.sha256(archive).hexdigest() != manifest['archiveSha256']:
    raise SystemExit('Archive checksum mismatch')
with tarfile.open(fileobj=io.BytesIO(archive), mode='r:gz') as tar:
    member = tar.getmember('cli-proxy-api')
    if not member.isfile():
        raise SystemExit('Invalid archive member')
    payload = tar.extractfile(member).read()
if hashlib.sha256(payload).hexdigest() != manifest['binarySha256']:
    raise SystemExit('Binary checksum mismatch')
staged = root / 'cli-proxy-api.download'
staged.write_bytes(payload)
staged.chmod(0o755)
staged.replace(binary)
print('CLIProxyAPI 8.0.27 ready')
