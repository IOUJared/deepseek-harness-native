#!/usr/bin/python3
"""Snapshot an external-runtime development binary/source archive, never user state."""
import argparse
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tarfile
import tomllib

BASE = Path(__file__).resolve().parents[1]
CARGO_SRC = BASE.parent / 'deepseek-harness-tauri/.cargo-home/registry/src'
SPEC = importlib.util.spec_from_file_location('development_launcher', BASE / 'scripts/development-launcher.py')
L = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(L)


def copy_new(source, target, mode=0o644, limit=64 * 1024 * 1024):
    if source.is_symlink():
        raise ValueError('Source symlinks are not packaged')
    before = source.stat()
    if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
        raise ValueError('Unexpected package source file')
    target.parent.mkdir(parents=True, exist_ok=True)
    with source.open('rb') as reader, target.open('xb') as writer:
        shutil.copyfileobj(reader, writer, 1024 * 1024)
    after = source.stat()
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise ValueError('Package source changed during snapshot')
    target.chmod(mode)


def licenses(metadata, payload):
    if (not isinstance(metadata, dict) or not isinstance(metadata.get('packages'), list)
            or not 1 <= len(metadata['packages']) <= 1024):
        raise ValueError('Invalid bounded Cargo metadata package list')
    for package in metadata['packages']:
        if not isinstance(package, dict): raise ValueError('Invalid Cargo package metadata')
        if package.get('source') is None: continue
        if (not isinstance(package.get('source'), str) or not package['source'].startswith('registry+')
                or not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', package.get('name', ''))
                or not re.fullmatch(r'[A-Za-z0-9.+_-]{1,100}', package.get('version', ''))):
            raise ValueError('Unsupported Cargo package identity')
        manifest = Path(package['manifest_path']).resolve(strict=True)
        relative = manifest.parent.relative_to(CARGO_SRC.resolve(strict=True))
        if (manifest.name != 'Cargo.toml' or len(relative.parts) != 2
                or manifest.parent.name != package['name'] + '-' + package['version']):
            raise ValueError('Cargo license source is not the expected cached crate')
    records = []
    for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
        if package['source'] is None:
            continue
        root = Path(package['manifest_path']).resolve().parent
        chosen = set()
        for path in root.iterdir():
            if path.is_file() and re.match(r'^(LICENSE|COPYING|NOTICE|COPYRIGHT)([._-].*)?$', path.name, re.I):
                chosen.add(path)
        if package.get('license_file'):
            explicit = Path(package['license_file'])
            if not explicit.is_absolute(): explicit = root / explicit
            if explicit.resolve().is_relative_to(root): chosen.add(explicit)
        copied = []
        for path in sorted(chosen):
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 1024 * 1024:
                continue
            relative = Path('licenses/rust') / (package['name'] + '-' + package['version']) / path.name
            copy_new(path, payload / relative, limit=1024 * 1024)
            copied.append(relative.as_posix())
        records.append({'name': package['name'], 'version': package['version'],
                        'declaredLicense': package['license'], 'licenseTexts': copied,
                        'source': 'https://crates.io/crates/' + package['name'] + '/' + package['version']})
    inventory = {'scope': 'Cargo registry metadata lockfile/target superset; source=None path dependencies excluded (notices retained in selected native sources); not exact compiled-link closure or legal certification',
                 'packageCount': len(records), 'packagesWithCopiedLicenseTexts': sum(bool(p['licenseTexts']) for p in records),
                 'packages': records}
    (payload / 'licenses').mkdir(exist_ok=True)
    with (payload / 'licenses/rust-inventory.json').open('x') as output:
        json.dump(inventory, output, indent=2); output.write('\n')
    return inventory


def qualified_native(path, expected_digest, app_root):
    """Check the selected PUBLIC UI qualification, not signatures or all backend source."""
    if app_root.is_symlink(): raise ValueError('Native app root symlink refused')
    value = json.loads(L.private_read(Path(path), 256 * 1024))
    if (not isinstance(value, dict)
            or value.get('scope') != 'native-control-layout-and-Harness-style-conversation'
            or value.get('defaultReleaseSha256') != expected_digest
            or value.get('parentPaintInspectionComplete') is not True
            or value.get('checkAllTargetsFeaturePassed') is not True):
        raise ValueError('Selected native qualification does not match this release')
    for field in ('defaultAppTestsPassed', 'featureAppTestsPassed', 'fixtureExampleTestsPassed'):
        if type(value.get(field)) is not int or value[field] <= 0:
            raise ValueError('Missing successful native test evidence')
    cases, real = value.get('publicCases'), value.get('realKeyless')
    if (not isinstance(cases, list) or not 1 <= len(cases) <= 128
            or any(not isinstance(c, dict) or c.get('passed') is not True
                   or type(c.get('exitCode')) is not int or c['exitCode'] != 0 for c in cases)
            or not isinstance(real, dict) or real.get('passed') is not True
            or type(real.get('exitCode')) is not int or real['exitCode'] != 0):
        raise ValueError('Missing successful native rendering/startup evidence')
    node_observed = value.get('systemNodeVersionObserved')
    if node_observed is not None and (not isinstance(node_observed, str)
            or re.fullmatch(r'v26\.[0-9]{1,4}\.[0-9]{1,4}', node_observed) is None):
        raise ValueError('Invalid observed system Node patch version')
    sources = value.get('sourceSha256')
    if not isinstance(sources, dict) or not 1 <= len(sources) <= 512 or 'src/ui.rs' not in sources:
        raise ValueError('Missing bounded native app-source pins')
    if value.get('formatCheckPassed') is not True:
        scoped = value.get('scopedFormatCheck')
        if (value.get('formatCheckPassed') is not False or not isinstance(scoped, dict)
                or scoped.get('passed') is not True or scoped.get('scope') != 'explicit-app-files'
                or type(scoped.get('exitCode')) is not int or scoped['exitCode'] != 0):
            raise ValueError('Missing explicit full or scoped native formatting evidence')
        files = scoped.get('files')
        if (not isinstance(files, list) or not 1 <= len(files) <= 512
                or any(not isinstance(name, str) or name not in sources
                       or not name.startswith(('src/', 'examples/')) or not name.endswith('.rs') for name in files)
                or len(set(files)) != len(files)):
            raise ValueError('Scoped formatting files must be unique qualified source pins')
    for name, digest in sources.items():
        if (not isinstance(name, str) or len(name) > 240 or '\\' in name
                or any(ord(c) < 32 or ord(c) == 127 for c in name)
                or Path(name).is_absolute() or '..' in Path(name).parts
                or Path(name).as_posix() != name
                or not (name in ('Cargo.toml', 'Cargo.lock') or name.startswith(('src/', 'examples/')))
                or not isinstance(digest, str) or re.fullmatch(r'[0-9a-f]{64}', digest) is None):
            raise ValueError('Invalid native source pin')
        source = app_root / name
        for item in (source, *source.parents):
            if item == app_root: break
            if item.is_symlink(): raise ValueError('Native source pin follows a symlink')
        if L.binary_digest(source) != digest:
            raise ValueError('Native source differs from selected qualification')
    return value


NATIVE_MODULES = ('app', 'core', 'transport')
NATIVE_FOLDERS = ('src', 'runtime', 'tests', 'examples', 'fixtures')
MAX_NATIVE_SOURCE_FILES = 4096
MAX_NATIVE_SOURCE_ENTRIES = 8192
MAX_NATIVE_SOURCE_BYTES = 128 * 1024 * 1024


def native_folders(module):
    # The dedicated App vendor tree contains its Cargo path patch, notices and provenance.
    # Share this policy with the output-overlap guard; do not admit other module vendors.
    return NATIVE_FOLDERS + (('vendor',) if module == 'app' else ())


def native_path(value, directory):
    """Require canonical absolute, own, nonsymlink native input paths."""
    raw = os.fspath(value)
    if (not isinstance(raw, str) or len(raw.encode('utf-8')) > 4096
            or any(ord(c) < 32 or ord(c) == 127 for c in raw)):
        raise ValueError('Invalid native input path')
    path = Path(raw)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError('Native inputs require absolute paths without parents')
    for item in (path, *path.parents):
        if item.is_symlink(): raise ValueError('Native input symlink refused')
    canonical = path.resolve(strict=True)
    info = canonical.stat()
    if (raw != str(canonical) or info.st_uid != os.getuid()
            or not (stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode))):
        raise ValueError('Native input must be canonical, own and the expected file type')
    return canonical


def native_inputs(source_root=None, native_binary=None):
    if (source_root is None) != (native_binary is None):
        raise ValueError('Source root and native binary must be supplied together')
    base = native_path(BASE if source_root is None else source_root, True)
    binary = native_path(BASE / 'app/target/release/dsh-native-app' if native_binary is None else native_binary, False)
    for module in NATIVE_MODULES:
        root = native_path(base / module, True)
        for name in ('Cargo.toml', 'Cargo.lock'): native_path(root / name, False)
        native_path(root / 'src', True)
    return base, binary


def native_source_files(base):
    """Single bounded source selection for copying and before/after inventories."""
    count = entries = 0
    for module in NATIVE_MODULES:
        root = base / module
        if root.is_symlink(): raise ValueError('Native module root symlink refused')
        chosen = [(root / name, 64 * 1024 * 1024) for name in ('Cargo.toml', 'Cargo.lock', 'LICENSE')]
        for folder in native_folders(module):
            directory = root / folder
            if directory.is_symlink(): raise ValueError('Native source root symlink refused')
            if not directory.is_dir(): continue
            for source in directory.rglob('*'):
                entries += 1
                if entries > MAX_NATIVE_SOURCE_ENTRIES: raise ValueError('Native source entry count exceeded bound')
                chosen.append((source, 8 * 1024 * 1024))
        for source, limit in sorted(chosen):
            if source.is_symlink(): raise ValueError('Native source symlink refused')
            if not source.exists(): continue
            if source.is_dir(): continue
            if (not source.is_file() or source.stat().st_uid != os.getuid()
                    or source.name.startswith('.env') or source.name in ('.credentials.yaml', '.npmrc')
                    or any(ord(c) < 32 or ord(c) == 127 for c in str(source.relative_to(base)))):
                raise ValueError('Private or unexpected native source file refused')
            count += 1
            if count > MAX_NATIVE_SOURCE_FILES: raise ValueError('Native source file count exceeded bound')
            yield source, limit


def copy_native_sources(base, payload):
    for source, limit in native_source_files(base):
        copy_new(source, payload / 'source/native' / source.relative_to(base), limit=limit)


def native_source_inventory(base):
    records = []; total = 0
    for source, limit in native_source_files(base):
        data = L.private_read(source, limit)
        total += len(data)
        if total > MAX_NATIVE_SOURCE_BYTES: raise ValueError('Native source bytes exceeded bound')
        records.append({'path': (Path('source/native') / source.relative_to(base)).as_posix(),
                        'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
    records.sort(key=lambda record: record['path'])
    return {'scope': 'Packaged native source snapshot; not an exact reproducible-build or complete third-party source certificate',
            'fileCount': len(records), 'files': records}


def snapshot_inventory(payload):
    result = native_source_inventory(payload / 'source/native')
    with (payload / 'source-snapshot.json').open('x') as writer:
        json.dump(result, writer, indent=2); writer.write('\n')
    return result


def build(output, metadata_path, qualification_path=None, *, source_root=None, native_binary=None):
    output = Path(output).absolute()
    allowed = (BASE / 'app/evidence').resolve()
    if output.exists() or output.is_symlink() or not output.parent.resolve().is_relative_to(allowed):
        raise ValueError('Package output must be new and below own evidence directory')
    selected_root, source_binary = native_inputs(source_root, native_binary)
    version = tomllib.loads(L.private_read(selected_root / 'app/Cargo.toml', 64 * 1024).decode('utf-8'))['package']['version']
    if not isinstance(version, str) or not re.fullmatch(r'[0-9][A-Za-z0-9.+-]{0,99}', version):
        raise ValueError('Unsafe native package version')
    if any(output.resolve().is_relative_to(selected_root / module / folder)
           for module in NATIVE_MODULES for folder in native_folders(module)):
        raise ValueError('Package output cannot overlap selected native sources')
    selected_sources = native_source_inventory(selected_root)
    before = L.binary_digest(source_binary)
    qualification = qualified_native(qualification_path, before, selected_root / 'app') if qualification_path else None
    output.mkdir(mode=0o700)
    node_observed = qualification.get('systemNodeVersionObserved') if qualification else None
    node_qualified = node_observed.removeprefix('v') if node_observed else None
    name = 'dsh-native-linux-x86_64-development-' + version + '-' + before[:12]
    payload = output / name; payload.mkdir(mode=0o700)
    copy_new(source_binary, payload / 'bin/dsh-native-app', 0o755)
    if L.binary_digest(payload / 'bin/dsh-native-app') != before:
        raise ValueError('Native binary snapshot mismatch')
    versions = subprocess.run(['/usr/bin/readelf', '--version-info', str(payload / 'bin/dsh-native-app')],
                              capture_output=True, timeout=10, check=True)
    if len(versions.stdout) > 1024 * 1024:
        raise ValueError('ELF version output exceeded bound')
    required = re.findall(rb'Name: GLIBC_(\d+)\.(\d+)', versions.stdout)
    minimum = max((int(a), int(b)) for a, b in required)
    manifest = {'application': L.APP, 'packageVersion': version if '-' in version else version + '-development', 'runtimeVersion': L.VERSION,
                'runtimeMode': 'external-explicit', 'architecture': 'x86_64',
                'minimumGlibc': '.'.join(map(str, minimum)), 'nativeSha256': before,
                'node': '/usr/bin/node', 'nodeSelectionMode': 'system-default-or-explicit-absolute',
                'nodeMajorAccepted': 26, 'nodeVersionQualified': node_qualified,
                'nodeQualificationScope': 'selected native keyless report observation only; null if absent',
                'primaryUi': 'Iced 0.14.0 / wgpu', 'electronIncluded': False, 'hostRuntimeIncluded': False,
                'standaloneInstaller': False, 'fullDesktopParity': False,
                'nativeUiQualificationIncluded': qualification is not None}
    with (payload / 'development-manifest.json').open('x') as writer:
        json.dump(manifest, writer, indent=2); writer.write('\n')
    copy_new(BASE / 'scripts/development-launcher.py', payload / 'launch.py', 0o755)
    instructions = L.private_read(BASE / 'docs/NATIVE-DEVELOPMENT-PACKAGE.md', 128 * 1024).decode('utf-8').split('## Qualified artifact', 1)[0]
    instructions += ('## This archive\n\nRead [development-manifest.json](<development-manifest.json>) for the executable digest, '
                     'glibc baseline and explicit external-runtime requirements; this digest is not a signature. '
                     '[source-snapshot.json](<source-snapshot.json>) lists the bundled native source, examples and fixtures, '
                     'not a certified reproducible build or complete dependency-source closure.\n\n')
    if qualification:
        instructions += ('[Included public UI qualification](<evidence/native-ui-qualification.json>) pins this executable '
                         'and the bundled app-source subset. It does not qualify physical input, real accounts/models, '
                         'rich inline diffs or full parity. Relocated startup checks are a separate distribution-side report.\n\n')
    else:
        instructions += ('This archive has no selected native UI qualification. '
                         '[Qualification status](<evidence/native-ui-qualification-status.json>) is explicitly unqualified; '
                         'relocated startup qualification must be run separately.\n\n')
    instructions += 'Self-contained runtime/Node, general Linux portability, complete redistribution-license review, installer/update ownership and full parity remain unfinished.\n'
    if qualification and qualification.get('formatCheckPassed') is False:
        instructions += ('\nThe full shared-App formatting check is not passed. Only the named source-pinned files in '
                         '`scopedFormatCheck` passed scoped rustfmt; full-App formatting is not qualified.\n')
    with (payload / 'README.md').open('x') as writer: writer.write(instructions)
    copy_new(BASE / 'core/LICENSE', payload / 'LICENSE')
    metadata = json.loads(L.private_read(Path(metadata_path), 8 * 1024 * 1024))
    inventory = licenses(metadata, payload)
    (payload / 'evidence').mkdir(exist_ok=True)
    with (payload / 'evidence/native-ui-qualification-status.json').open('x') as writer:
        json.dump({'status': 'selected-report-included' if qualification else 'unqualified',
                   'nativeUiQualificationIncluded': qualification is not None,
                   'relocatedStartupQualified': False,
                   'scope': 'Snapshot consistency is not UI, provider or relocated startup qualification'}, writer, indent=2)
        writer.write('\n')
    if qualification:
        copy_new(Path(qualification_path), payload / 'evidence/native-ui-qualification.json', limit=256 * 1024)
    copy_native_sources(selected_root, payload)
    snapshot = snapshot_inventory(payload)
    if native_source_inventory(selected_root) != selected_sources or snapshot != selected_sources:
        raise ValueError('Selected native sources changed during snapshot')
    if qualification and qualified_native(payload / 'evidence/native-ui-qualification.json', before,
                                          payload / 'source/native/app') != qualification:
        raise ValueError('Qualification changed during snapshot')
    if L.binary_digest(source_binary) != before:
        raise ValueError('Release binary changed during packaging')
    archive = output / (name + '.tar.gz')
    with archive.open('xb') as file:
        with gzip.GzipFile(filename='', mode='wb', fileobj=file, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as tar:
                for path in [payload] + sorted(payload.rglob('*')):
                    if path.is_symlink(): raise ValueError('Payload symlink refused')
                    info = tar.gettarinfo(str(path), arcname=path.relative_to(output).as_posix())
                    info.uid = info.gid = 0; info.uname = info.gname = ''; info.mtime = 0
                    info.mode = (0o755 if path.is_dir() or os.access(path, os.X_OK) else 0o644)
                    if path.is_file():
                        with path.open('rb') as reader: tar.addfile(info, reader)
                    else: tar.addfile(info)
    archive.chmod(0o600)
    result = {'status': 'built-external-runtime-development-package', 'archive': archive.name,
              'nativeSha256': before, 'archiveSha256': L.binary_digest(archive), 'archiveBytes': archive.stat().st_size,
              'minimumGlibc': manifest['minimumGlibc'], 'runtimeBundled': False, 'nodeBundled': False,
              'nativeUiQualificationIncluded': qualification is not None,
              'nativeSourceSnapshotFiles': snapshot['fileCount'],
              'nativeSourceSnapshotSha256': L.binary_digest(payload / 'source-snapshot.json'),
              'rustMetadataPackageCount': inventory['packageCount'],
              'rustPackagesWithCopiedLicenseTexts': inventory['packagesWithCopiedLicenseTexts'],
              'standaloneOrGeneralLinuxPortabilityClaimed': False, 'publicRedistributionLicenseReviewComplete': False}
    with (output / 'build.json').open('x') as writer:
        json.dump(result, writer, indent=2); writer.write('\n')
    print(json.dumps(result))
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument('--output', required=True); parser.add_argument('--rust-metadata', required=True)
    parser.add_argument('--native-qualification', help='Optional PUBLIC UI qualification matching the binary and bundled app-source pins')
    parser.add_argument('--source-root', help='Absolute canonical existing native app/core/transport source root; requires --native-binary')
    parser.add_argument('--native-binary', help='Absolute canonical existing regular binary; requires --source-root')
    options = parser.parse_args()
    build(options.output, options.rust_metadata, options.native_qualification,
          source_root=options.source_root, native_binary=options.native_binary)
