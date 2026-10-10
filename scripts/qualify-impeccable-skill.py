#!/usr/bin/env python3
"""Qualify pinned skill loading through the existing pidfd-owned alpha profile runner."""
import argparse
import asyncio
import importlib.util
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("owned_skill_qualification", ROOT / "scripts/qualify-lazy-account.py")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
EXPECTED = {"native-repository": "impeccable-rust", "desktop-workspace": "impeccable-rust", "backend-repository": "harness-verification"}
EXPECTED_HASHES = {"impeccable-rust": hashlib.sha256((ROOT / ".agents/skills/impeccable-rust/SKILL.md").read_bytes()).hexdigest(),
                   "harness-verification": hashlib.sha256((ROOT.parent / "deepseek-harness-linux/.agents/skills/harness-verification/SKILL.md").read_bytes()).hexdigest()}


def valid_skill(report):
    """Reject partial loads, wrong scope, wakes and metadata-only evidence."""
    try:
        facts = report["facts"]
        counts = report["counts"]
        return (report["status"] == "passed" and report["phase"] == "complete"
                and report["fatal"] is False and report["runtime"] == "0.2.1-alpha.1"
                and report["providerSubstituted"] is False and report["modelPromptsRequested"] is False
                and set(counts) == {"turnStarts", "stepStarts", "requests", "unexpectedWake"}
                and all(type(value) is int and value == 0 for value in counts.values())
                and len(facts) == 3 and {fact["name"]: fact["skill"] for fact in facts} == EXPECTED
                and all(fact["catalog"] is True and fact["bodyLoaded"] is True and fact["toolLoaded"] is True
                        and fact["instructionSha256"] == EXPECTED_HASHES[fact["skill"]] for fact in facts))
    except (KeyError, TypeError, AttributeError):
        return False


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args()
    raise SystemExit(asyncio.run(RUNNER.run(options, script=ROOT / "scripts/qualify-impeccable-skill.mjs",
        report_name="skill.json", report_key="skill", evidence_env="DSH_NATIVE_SKILL_EVIDENCE",
        validator=valid_skill, scope="Owned exact-alpha profile, real scoped catalog and full skill tool loading; no GUI/provider execution")))
