import copy
import importlib.util
from pathlib import Path
import unittest
SPEC = importlib.util.spec_from_file_location('lazy_account', Path(__file__).with_name('qualify-lazy-account.py'))
QUALIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUALIFY)


def report():
    ready = dict(watch=0, state=0, session=0, live=0, maxLive=0, ended=0, accountEvents=0, platformEvents=0)
    return dict(status='passed', phase='complete', fatal=False, processWideNetworkTrace=False,
                providerHotReplacementQualified=False, scriptedSignIn=False, scriptedModelOrCatalog=False,
                scriptedCredentialSave=False,
                counts=dict(watch=2, state=6, session=2, live=0, maxLive=1, ended=2),
                events=dict(account=2, platform=4, nonnullPlatform=0, observationErrors=0),
                snapshots=dict(ready=ready, metadata={**ready, 'state': 2},
                               subscribed={**ready, 'state': 4, 'watch': 1, 'session': 1, 'live': 1, 'maxLive': 1, 'accountEvents': 1, 'platformEvents': 1},
                               unsubscribed={**ready, 'state': 4, 'watch': 1, 'session': 1, 'maxLive': 1, 'ended': 1, 'accountEvents': 1, 'platformEvents': 2},
                               stopped={**ready, 'state': 6, 'watch': 2, 'session': 2, 'maxLive': 1, 'ended': 2, 'accountEvents': 2, 'platformEvents': 4}))


class Qualification(unittest.TestCase):
    def test_exact_narrow_success_is_accepted(self):
        self.assertTrue(QUALIFY.valid_account(report()))

    def test_eager_reads_or_secret_projection_never_qualify(self):
        for path, value in [(('snapshots', 'ready', 'watch'), 1), (('snapshots', 'ready', 'state'), 1),
                            (('snapshots', 'metadata', 'session'), 1), (('events', 'nonnullPlatform'), 1),
                            (('events', 'observationErrors'), 1)]:
            data = report()
            target = data
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = value
            self.assertFalse(QUALIFY.valid_account(data), path)

    def test_duplicate_or_unretired_watch_never_qualifies(self):
        for key, value in [('maxLive', 2), ('live', 1), ('ended', 1), ('watch', 3), ('session', 3)]:
            data = report()
            data['counts'][key] = value
            self.assertFalse(QUALIFY.valid_account(data), key)

    def test_partial_wrong_types_and_overclaims_are_rejected(self):
        self.assertFalse(QUALIFY.valid_account({}))
        for key in ['processWideNetworkTrace', 'providerHotReplacementQualified', 'scriptedCredentialSave', 'scriptedSignIn', 'scriptedModelOrCatalog', 'fatal']:
            data = report()
            data[key] = True
            self.assertFalse(QUALIFY.valid_account(data), key)
        for value in [True, False, -1, 1.0, '1', None]:
            data = copy.deepcopy(report())
            data['counts']['maxLive'] = value
            self.assertFalse(QUALIFY.valid_account(data), value)


if __name__ == '__main__':
    unittest.main()
