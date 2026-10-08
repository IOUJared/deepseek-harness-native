#!/usr/bin/python3
"""Verify/extract an own development archive into a new evidence directory; no installer."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import tarfile

BASE = Path(__file__).resolve().parents[1]


def validate_members(members, root):
    if not 1 <= len(members) <= 2048:
        raise ValueError('Archive member count exceeds bound')
    seen, total, root_directory = set(), 0, False
    for member in members:
        name = member.name
        parts = PurePosixPath(name).parts
        if (not parts or parts[0] != root or PurePosixPath(name).is_absolute()
                or '..' in parts or PurePosixPath(name).as_posix() != name
                or '\\' in name or any(ord(c) < 32 or ord(c) == 127 for c in name)
                or name in seen or not (member.isdir() or member.isreg())
                or member.mode not in (0o644, 0o755) or member.size < 0
                or (member.isdir() and (member.mode != 0o755 or member.size != 0))
                or (name == root and not member.isdir())):
            raise ValueError('Unsafe or duplicate archive member')
        root_directory |= name == root and member.isdir()
        seen.add(name); total += member.size
        if total > 64 * 1024 * 1024:
            raise ValueError('Expanded archive exceeds bound')
    if not root_directory:
        raise ValueError('Missing expected archive root directory')
    return total


def snapshot(root, *, normalize_modes=False):
    if root.is_symlink() or not root.is_dir():
        raise ValueError('Expected snapshot root directory')
    result = {}
    for path in [root] + sorted(root.rglob('*')):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError('Unexpected snapshot object')
        name = path.relative_to(root.parent).as_posix()
        digest = None if path.is_dir() else hashlib.sha256(path.read_bytes()).hexdigest()
        # Match the builder's os.access criterion, not just owner execute bits.
        if normalize_modes:
            mode = 0o755 if path.is_dir() or os.access(path, os.X_OK) else 0o644
        else:
            mode = stat.S_IMODE(path.stat().st_mode)
        result[name] = (digest, mode)
    return result


def tree(root):
    return {name: digest for name, (digest, _) in snapshot(root).items()}


def validate_inventory(extracted, extracted_tree):
    inventory = json.loads((extracted / 'source-snapshot.json').read_text())
    if (not isinstance(inventory, dict) or type(inventory.get('fileCount')) is not int
            or not isinstance(inventory.get('files'), list)):
        raise ValueError('Invalid source snapshot inventory')
    records = inventory['files']
    prefix = extracted.name + '/'
    source_paths = {name[len(prefix):] for name, (digest, _) in extracted_tree.items()
                    if name.startswith(prefix + 'source/native/') and digest is not None}
    if (not 0 <= len(records) <= 2048 or len(records) != inventory['fileCount']
            or len(records) != len(source_paths)):
        raise ValueError('Incomplete source snapshot inventory')
    seen = set()
    for record in records:
        if not isinstance(record, dict):
            raise ValueError('Invalid source snapshot record')
        name = record.get('path')
        if not isinstance(name, str):
            raise ValueError('Invalid source snapshot path')
        parts = PurePosixPath(name).parts
        if (len(parts) < 3 or parts[:2] != ('source', 'native')
                or PurePosixPath(name).as_posix() != name or '..' in parts or '\\' in name
                or any(ord(c) < 32 or ord(c) == 127 for c in name)
                or name not in source_paths or name in seen
                or type(record.get('bytes')) is not int or record['bytes'] < 0
                or not isinstance(record.get('sha256'), str)
                or record['sha256'] != extracted_tree[prefix + name][0]):
            raise ValueError('Source inventory mismatch')
        seen.add(name)
    if seen != source_paths:
        raise ValueError('Incomplete source snapshot inventory')
    # Read only identities already checked against the finite extracted snapshot.
    for record in records:
        data = (extracted / record['path']).read_bytes()
        if len(data) != record['bytes'] or hashlib.sha256(data).hexdigest() != record['sha256']:
            raise ValueError('Source inventory mismatch')
    return records


def normalized_data_filter(member, destination):
    filtered = tarfile.data_filter(member, destination)
    # The data filter deliberately drops directory modes. Restore our validated
    # 0755 so the extracted package is normalized independently of the umask.
    return filtered.replace(mode=0o755) if filtered.isdir() else filtered


def verify(archive, destination, expected):
    archive, expected = Path(archive), Path(expected)
    destination = Path(destination).absolute()
    allowed = (BASE / 'app/evidence').resolve()
    if (destination.exists() or destination.is_symlink()
            or not destination.parent.resolve().is_relative_to(allowed)
            or archive.is_symlink() or expected.is_symlink()):
        raise ValueError('Only a new owned evidence extraction is accepted')
    expected_tree = snapshot(expected, normalize_modes=True)
    for name in ('launch.py', 'bin/dsh-native-app'):
        entry = expected_tree.get(expected.name + '/' + name)
        if entry is None or entry[0] is None or entry[1] != 0o755:
            raise ValueError('Required package entry point is not executable')
    with tarfile.open(archive, 'r:gz') as handle:
        # Own builder output only: these bounds apply after getmembers() parses
        # the archive, not as adversarial streaming/resource protection.
        members = handle.getmembers(); total = validate_members(members, expected.name)
        actual = {}
        for member in members:
            if member.isdir(): actual[member.name] = (None, member.mode)
            else:
                with handle.extractfile(member) as reader:
                    digest = hashlib.sha256(); size = 0
                    while chunk := reader.read(1024 * 1024):
                        size += len(chunk); digest.update(chunk)
                    if size != member.size: raise ValueError('Incomplete archive member')
                actual[member.name] = (digest.hexdigest(), member.mode)
        if actual != expected_tree: raise ValueError('Archive differs from own generated payload')
        destination.mkdir(mode=0o700)
        handle.extractall(destination, members=members, filter=normalized_data_filter)
    extracted = destination / expected.name
    extracted_tree = snapshot(extracted)
    if extracted_tree != expected_tree: raise ValueError('Extracted payload differs')
    records = validate_inventory(extracted, extracted_tree)
    report = dict(status='passed', archiveSha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
                  memberCount=len(members), expandedBytes=total, nativeSourceFiles=len(records),
                  completeGeneratedPayloadAndExtractedHashesMatch=True, sourceInventoryComplete=True,
                  noLinksSpecialFilesDuplicateOrUnsafeMembers=True, signatureOrAdversarialSourceProtectionClaimed=False,
                  extractedPackage=str(extracted))
    with (destination / 'extraction-result.json').open('x') as writer:
        json.dump(report, writer, indent=2); writer.write('\n')
    (destination / 'extraction-result.json').chmod(0o600)
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument('--archive', required=True); parser.add_argument('--destination', required=True)
    parser.add_argument('--expected-payload', required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.archive, args.destination, args.expected_payload)))
