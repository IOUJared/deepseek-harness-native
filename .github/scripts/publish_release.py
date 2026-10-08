#!/usr/bin/env python3
"""Publish tested assets as a draft first; never overwrite a published release."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile


def sha256(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def verify_assets(directory, tag, commit):
    """Verify the original archive/checksum and its embedded target, without extraction."""
    name = f'dsh-native-linux-x86_64-{tag}'
    archive = directory / f'{name}.tar.gz'
    expected = {archive.name, 'SHA256SUMS'}
    if {path.name for path in directory.iterdir()} != expected:
        raise ValueError('Missing or unexpected release assets')
    checksum = (directory / 'SHA256SUMS').read_text()
    if checksum != f'{sha256(archive)}  {archive.name}\n':
        raise ValueError('Release archive checksum mismatch')
    with tarfile.open(archive, 'r:gz') as tar:
        member = tar.getmember(f'{name}/build-info.json')
        if not member.isfile() or member.size > 64 * 1024:
            raise ValueError('Invalid embedded build metadata')
        with tar.extractfile(member) as source:
            info = json.load(source)
    if info.get('commit') != commit or info.get('releaseTag') != tag or info.get('developmentPrerelease') is not True:
        raise ValueError('Release metadata does not match the tested commit')


def main():
    repo, tag, commit = (os.environ[key] for key in ('GH_REPO', 'RELEASE_TAG', 'GITHUB_SHA'))
    if not re.fullmatch(r'v\d+\.\d+\.\d+-dev\.[1-9]\d*', tag) or not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('Invalid release target')
    assets = Path('release-output')
    archive = assets / f'dsh-native-linux-x86_64-{tag}.tar.gz'
    notes = assets / 'release-notes.md'
    files = [str(archive), str(assets / 'SHA256SUMS')]

    def gh(*args):
        return subprocess.check_output(['gh', *args], text=True).strip()

    refs = json.loads(gh('api', f'repos/{repo}/git/matching-refs/tags/{tag}'))
    matching = [ref for ref in refs if ref['ref'] == f'refs/tags/{tag}']
    if matching and (len(matching) != 1 or matching[0]['object']['type'] != 'commit' or matching[0]['object']['sha'] != commit):
        raise ValueError('Existing release tag targets another commit or is not lightweight')
    state = subprocess.run(['gh', 'release', 'view', tag, '--repo', repo, '--json',
                            'isDraft,isPrerelease,targetCommitish,assets'], text=True, capture_output=True)
    release = json.loads(state.stdout) if state.returncode == 0 else None
    expected_names = {archive.name, 'SHA256SUMS'}
    if release is not None:
        if not release['isPrerelease'] or release['targetCommitish'] != commit:
            raise ValueError('Existing release does not match this development build')
        if not release['isDraft']:
            if not matching or {asset['name'] for asset in release['assets']} != expected_names:
                raise ValueError('Published release is missing its expected assets; not overwritten')
            with tempfile.TemporaryDirectory(prefix='native-published-assets-') as temporary:
                directory = Path(temporary)
                gh('release', 'download', tag, '--repo', repo, '--dir', str(directory))
                verify_assets(directory, tag, commit)
            print('Published release and original assets verified; nothing overwritten.')
            return
        gh('release', 'upload', tag, *files, '--repo', repo, '--clobber')
    else:
        gh('release', 'create', tag, *files, '--repo', repo, '--target', commit,
           '--title', f'Harness Native {tag}', '--draft', '--prerelease', '--latest=false', '--notes-file', str(notes))
    with tempfile.TemporaryDirectory(prefix='native-draft-assets-') as temporary:
        directory = Path(temporary)
        gh('release', 'download', tag, '--repo', repo, '--dir', str(directory))
        verify_assets(directory, tag, commit)
    gh('release', 'edit', tag, '--repo', repo, '--draft=false', '--prerelease', '--latest=false')
    print(f'Published verified development release {tag} for {commit}.')


if __name__ == '__main__':
    main()
