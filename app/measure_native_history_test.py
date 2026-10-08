"""Pure sampling-accounting tests; no process launch, GUI, Host or desktop actions."""
import unittest
import measure_native_history as measure

class Accounting(unittest.TestCase):
    def test_nearest_rank_percentiles_not_interpolated(self):
        self.assertIsNone(measure.percentile([], .95))
        self.assertEqual(measure.percentile([3,1,2], .5),2)
        self.assertEqual(measure.percentile([3,1,2], .95),3)

    def test_cpu_denominator_and_pss_ignore_unavailable_observation(self):
        samples=[dict(monotonicNs=0,unixTimestampNs=10,cpuTicks=20,rssKiB=2048,pssKiB=None,sampleDurationNs=1,directChildrenObserved=[]),dict(monotonicNs=2_000_000_000,unixTimestampNs=30,cpuTicks=20+measure.TICKS,rssKiB=3072,pssKiB=1024,sampleDurationNs=2,directChildrenObserved=[])]
        report=measure.resources(samples)
        self.assertEqual(report['oneCoreCpuPercent'],50)
        self.assertEqual(report['peakRssMiB'],3)
        self.assertEqual(report['medianPssMiB'],1)
        self.assertEqual(report['observedSeconds'],2)
        self.assertTrue(report['ownedDirectChildrenAbsentInSamples'])

    def test_empty_or_short_idle_is_not_zero_cpu_evidence(self):
        report=measure.resources([])
        self.assertIsNone(report['oneCoreCpuPercent'])
        self.assertIsNone(report['peakRssMiB'])
        self.assertEqual(report['samples'],0)

    def test_idle_marker_brackets_only_completed_hold(self):
        markers=[dict(stage='native-observed',unixTimestampNs=5),dict(stage='idle-begin',unixTimestampNs=10),dict(stage='idle-end',unixTimestampNs=20),dict(stage='paint-only',unixTimestampNs=25)]
        self.assertEqual(measure.idle_interval(markers),(10,20))
        self.assertEqual(measure.idle_interval(markers[:1]),(None,None))
        samples=[dict(monotonicNs=v*1000000,unixTimestampNs=v,cpuTicks=0,rssKiB=1024,pssKiB=512,sampleDurationNs=1,directChildrenObserved=[99] if v==25 else []) for v in [5,10,15,20,25]]
        idle=measure.resources(samples,(10,20));self.assertEqual(idle['samples'],3)
        self.assertTrue(idle['ownedDirectChildrenAbsentInSamples'])
        self.assertFalse(measure.resources(samples)['ownedDirectChildrenAbsentInSamples'])

if __name__=='__main__':unittest.main()
