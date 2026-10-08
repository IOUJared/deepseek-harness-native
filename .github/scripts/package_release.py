#!/usr/bin/env python3
"""Package one tested Git checkout as an external-runtime Linux development build."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def release_identity(manifest, commit, run_number):
    """Return a unique development tag from the app version, Git SHA and CI run."""
    version = tomllib.loads(manifest)['package']['version']
    if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', version):
        raise ValueError('App version must be a plain semantic version')
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('Expected a full Git commit SHA')
    if not re.fullmatch(r'[1-9]\d*', run_number):
        raise ValueError('Expected a positive GitHub run number')
    return {'packageVersion': version, 'releaseTag': f'v{version}-dev.{run_number}',
            'commit': commit, 'runNumber': run_number}


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def elf_requirements(version_info, dynamic_info):
    """Report measured GNU libc symbol requirements and dynamic dependencies."""
    versions = {(int(major), int(minor)) for major, minor in re.findall(r'Name: GLIBC_(\d+)\.(\d+)', version_info)}
    if not versions:
        raise ValueError('Missing GNU libc symbol version requirements')
    return {'minimumGlibcSymbols': '.'.join(map(str, max(versions))),
            'dynamicDependencies': re.findall(r'\(NEEDED\).*?\[(.*?)\]', dynamic_info)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    commit = command('git', 'rev-parse', 'HEAD')
    if commit != os.environ['GITHUB_SHA']:
        raise ValueError('Checkout differs from the triggering commit')
    identity = release_identity((ROOT / 'app/Cargo.toml').read_text(), commit, os.environ['GITHUB_RUN_NUMBER'])
    binary = args.binary.resolve(strict=True)
    if not binary.is_file() or binary.is_symlink() or not os.access(binary, os.X_OK):
        raise ValueError('Expected a built executable')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    name = f"dsh-native-linux-x86_64-{identity['releaseTag']}"
    payload = output / name
    (payload / 'bin').mkdir(parents=True)
    packaged = payload / 'bin/dsh-native-app'
    packaged.write_bytes(binary.read_bytes())
    packaged.chmod(0o755)
    (payload / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    requirements = elf_requirements(command('readelf', '--version-info', str(binary)),
                                    command('readelf', '-d', str(binary)))
    spec = importlib.util.spec_from_file_location('native_package_licenses', ROOT / 'scripts/build-development-package.py')
    licenses = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(licenses)
    licenses.CARGO_SRC = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / 'registry/src'
    metadata = json.loads(command('cargo', 'metadata', '--locked', '--offline', '--format-version', '1',
                                  '--manifest-path', str(ROOT / 'app/Cargo.toml')))
    inventory = licenses.licenses(metadata, payload)
    subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={payload / "source.tar.gz"}', commit], cwd=ROOT, check=True)
    instructions = '''# Harness Native — development build

Linux x86_64 GNU/Linux binary; built on Ubuntu 24.04. Not an installer, AppImage,
self-contained backend, generally portable Linux build or finished desktop release.
See build-info.json for measured glibc symbol requirements and dynamic libraries.

Requires a separately built @deepseek-ai/dsh 0.2.1-alpha.1 backend with its base+web
composition, Node 26 (26.10.0 tested), Python 3.11+ for optional tooling, native
Wayland, usable GPU/EGL/Vulkan drivers, xkbcommon and DejaVu system fonts.
The backend, Node, private profiles and credentials are NOT included.

Run from this extracted directory, replacing every absolute path:

./bin/dsh-native-app --runtime /absolute/deepseek-harness-linux/apps/cli \\
  --expected-version 0.2.1-alpha.1 --home /absolute/separate-native-home \\
  --user-home /absolute/user-home --cwd /absolute/workspace

Optional: --node /absolute/node-executable (default /usr/bin/node).
Keep the native home separate from existing Harness profiles.

CI checks compilation, headless regressions and --help, not rendered Wayland,
physical input, accessibility, real model/login behavior or complete feature parity.
Rust license inventory includes the lockfile/target superset, not legal certification.
Source snapshot comes from the exact tagged Git commit; no local evidence is included.
'''
    (payload / 'README.txt').write_text(instructions)
    info = {**identity, **requirements, 'target': 'x86_64-unknown-linux-gnu',
            'runner': 'ubuntu-24.04', 'rust': command('rustc', '--version'),
            'node': command('/usr/bin/node', '--version'), 'binarySha256': digest(packaged),
            'rustLicensePackageCount': inventory['packageCount'], 'developmentPrerelease': True,
            'externalBackendRequired': '0.2.1-alpha.1', 'guiOrRealModelVerified': False}
    (payload / 'build-info.json').write_text(json.dumps(info, indent=2) + '\n')
    archive = output / f'{name}.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        tar.add(payload, arcname=name)
    (output / 'SHA256SUMS').write_text(f'{digest(archive)}  {archive.name}\n')
    notes = f"""Development prerelease from `{commit}` after successful Linux build and headless checks.

- Package version: `{identity['packageVersion']}`; CI build: `{identity['runNumber']}`.
- Ubuntu 24.04 / Linux x86_64; required glibc symbols: `{requirements['minimumGlibcSymbols']}`.
- Requires the separate compatible alpha backend and Node 26. Not self-contained.
- Includes executable, exact-commit source, Rust license inventory and SHA-256 checksum.
- No real-provider, account, physical-input or rendered-GUI validation in this workflow.
"""
    (output / 'release-notes.md').write_text(notes)
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as result:
            result.write(f"tag={identity['releaseTag']}\n")
    print(json.dumps(info, indent=2))


if __name__ == '__main__':
    main()
