import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('publish_release', Path(__file__).with_name('publish_release.py'))
PUBLISH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISH)
TAG, SHA = 'v0.1.0-dev.1', 'a' * 40


def assets(directory, commit=SHA):
    name = f'dsh-native-linux-x86_64-{TAG}'
    archive = directory / f'{name}.tar.gz'
    info = json.dumps({'commit': commit, 'releaseTag': TAG, 'developmentPrerelease': True}).encode()
    with tarfile.open(archive, 'w:gz') as tar:
        member = tarfile.TarInfo(f'{name}/build-info.json')
        member.size = len(info)
        tar.addfile(member, io.BytesIO(info))
    (directory / 'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')
    return archive


class PublishedAssetsTests(unittest.TestCase):
    def test_original_assets_are_verified_independent_of_new_rebuild_timestamps(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            assets(directory)
            PUBLISH.verify_assets(directory, TAG, SHA)

    def test_missing_asset_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(ValueError):
                PUBLISH.verify_assets(Path(temporary), TAG, SHA)

    def test_corrupt_archive_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            archive = assets(directory)
            with archive.open('ab') as source:
                source.write(b'corruption')
            with self.assertRaises(ValueError):
                PUBLISH.verify_assets(directory, TAG, SHA)

    def test_another_commit_is_rejected_even_with_valid_checksum(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            assets(directory, 'b' * 40)
            with self.assertRaises(ValueError):
                PUBLISH.verify_assets(directory, TAG, SHA)

    def test_missing_checksum_fails_without_publishing(self):
        with patch.dict('os.environ', {'GH_REPO': 'owner/repo', 'RELEASE_TAG': TAG, 'GITHUB_SHA': SHA}):
            with patch.object(PUBLISH.subprocess, 'check_output', return_value='[]') as gh:
                state = type('State', (), {'returncode': 0, 'stdout': json.dumps({
                    'isDraft': False, 'isPrerelease': True, 'targetCommitish': SHA,
                    'assets': [{'name': f'dsh-native-linux-x86_64-{TAG}.tar.gz'}]})})()
                with patch.object(PUBLISH.subprocess, 'run', return_value=state):
                    with self.assertRaises(ValueError):
                        PUBLISH.main()
                self.assertEqual(gh.call_count, 1)

    def test_foreign_existing_tag_fails_before_release_mutation(self):
        value = [{'ref': f'refs/tags/{TAG}', 'object': {'type': 'commit', 'sha': 'b' * 40}}]
        with patch.dict('os.environ', {'GH_REPO': 'owner/repo', 'RELEASE_TAG': TAG, 'GITHUB_SHA': SHA}):
            with patch.object(PUBLISH.subprocess, 'check_output', return_value=json.dumps(value)) as gh:
                with self.assertRaises(ValueError):
                    PUBLISH.main()
                self.assertEqual(gh.call_count, 1)


if __name__ == '__main__':
    unittest.main()
