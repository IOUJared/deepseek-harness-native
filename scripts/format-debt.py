"""First-party Rust formatting-debt ratchet.

Parts of this repository are not rustfmt-clean, and `AGENTS.md` forbids rewriting
all shared Rust files to resolve local formatting debt. This check therefore keeps
the debt visible and measurable without silently waiving it:

- It runs the same scoped `cargo fmt -- --check` as the verification runner, so
  the pinned third-party tree `app/vendor/winit` is never claimed as ours.
- It fails when a file that was clean becomes unformatted, or when a new file is
  added unformatted.
- Files already recorded in the baseline stay listed, with exact hunk counts, so
  the debt can only shrink deliberately.

Update the baseline only for an intentional, reviewed change:

    python3 -B scripts/format-debt.py --update
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
BASELINE = Path(__file__).resolve().with_name("format-debt-baseline.json")
MANIFESTS = ("core/Cargo.toml", "transport/Cargo.toml", "app/Cargo.toml")
DIFF_LINE = re.compile(r"^Diff in (.+):(\d+):$")


def parse_diff_paths(text, root=ROOT):
    """Map normalized first-party source paths to their rustfmt diff-hunk count."""
    counts = {}
    for line in text.splitlines():
        match = DIFF_LINE.match(line.strip())
        if not match:
            continue
        path = Path(os.path.normpath(match.group(1)))
        try:
            name = path.relative_to(root).as_posix()
        except ValueError:
            name = path.as_posix()
        counts[name] = counts.get(name, 0) + 1
    return counts


def collect(manifests=MANIFESTS, root=ROOT, env=None):
    """Run the scoped check per first-party package and return path->hunk counts."""
    counts = {}
    failures = []
    for manifest in manifests:
        result = subprocess.run(
            ["cargo", "fmt", "--manifest-path", manifest, "--", "--check"],
            cwd=root, env=env, capture_output=True, text=True)
        found = parse_diff_paths(result.stdout + result.stderr, root)
        counts.update(found)
        if result.returncode not in (0, 1) and not found:
            failures.append((manifest, result.returncode,
                             (result.stderr or result.stdout).strip().splitlines()[:3]))
    return counts, failures


def evaluate(current, baseline):
    """Return (new offenders, resolved baseline entries) as sorted lists."""
    known = set(baseline)
    return sorted(set(current) - known), sorted(known - set(current))


def load_baseline(path=BASELINE):
    if not path.is_file():
        return []
    return json.loads(path.read_text(encoding="utf-8"))["files"]


def write_baseline(files, path=BASELINE):
    payload = {
        "note": "First-party Rust files that are not rustfmt-clean. Recorded debt, "
                "not approval: new files must not be added here without review.",
        "files": sorted(files),
    }
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update", action="store_true",
                        help="Record the current first-party debt as the baseline")
    parser.add_argument("--baseline", type=Path, default=BASELINE)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)

    current, failures = collect(root=args.root)
    for manifest, code, lines in failures:
        print(f"cargo fmt could not run for {manifest} (exit {code}): {' / '.join(lines)}",
              file=sys.stderr)
    if failures:
        return 2
    if args.update:
        write_baseline(current, args.baseline)
        print(f"Recorded {len(current)} first-party file(s) in formatting-debt baseline.")
        return 0

    baseline = load_baseline(args.baseline)
    new, resolved = evaluate(current, baseline)
    hunks = sum(current.values())
    print(f"First-party formatting debt: {len(current)} file(s), {hunks} rustfmt diff hunk(s).")
    if resolved:
        print(f"Resolved since the baseline (remove with --update): {len(resolved)} file(s).")
    if new:
        print("New unformatted file(s) — run cargo fmt for the owning package:")
        for name in new:
            print(f"  {name}")
        return 1
    print("No new formatting debt.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
