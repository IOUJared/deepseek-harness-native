# Performance iteration

Follow this when the job is to make working Rust faster or leaner: an optimization run, a speedup target, or beating another crate. Speed work is a search. Measure, change one thing, prove it is still right, race it against the baseline, keep or revert, and repeat until the gains converge.

The rules here have three strengths. State in the report which of them applied.

- **Firm:** the benchmark contract (SKILL.md, section 4) and the oracle. A change that breaks either one is not a result. Report it as a failed experiment.
- **Default:** the numbers below. Use them unless the user sets others.
- **Judgment:** which hypothesis to try next, and whether a gain is worth its code. Base the call on measurements and write down the reason.

## 1. Set up

Finish setup before changing production code.

1. Name the primary metric: latency, throughput, goodput, instructions, allocations, peak memory, startup time, or binary size. Name the secondary metrics that must hold.
2. Name the oracle. For an optimization this is the old implementation kept in the tree (section 2 of SKILL.md), a reference implementation, or properties and invariants. When speed can trade against output quality (accuracy, loss, compression ratio), quality becomes a metric with its own tolerance.
3. Write the workload matrix from real use: small, typical, large, pathological, cold, and warm inputs.
4. Commit the benchmarks and the matrix. That commit is the **baseline**, and the contract is frozen there.
5. Measure the baseline. Check out the baseline commit in its own worktree and run the matrix with the same command you will use on candidates, through `scripts/impeccable bench-guard <baseline> -- <command>`. Repeat until the median and spread are stable. Record the toolchain, profile, CPU, features, and inputs.

| Default | Value |
|---------|-------|
| Target | at least 1.2x on the primary metric, against the baseline |
| Regression | at most 3% on any workload in the matrix |
| Memory | at most 5% growth in peak memory |
| Quality | within the baseline's run-to-run variation |

The target is a floor, not a stopping point. When the contract and the oracle cannot both hold at the target, report the best result that keeps them.

## 2. Loop

Profile before you guess. Use `samply record` (Linux and macOS) or `perf record` (Linux) for CPU time, `dhat` for allocations, and gungraun's Callgrind output for instruction and cache counts.

Write each **hypothesis** down before trying it: the code boundary, why it should move the metric, the expected size, the correctness risk, and the measurement that would refute it. Rank by expected value. Search across these lenses: work that can be removed, algorithmic complexity, repeated computation, allocation and copying, data layout (section 4 of SKILL.md), branches and vectorization, parallelism and contention, I/O and FFI boundaries, and dependency overhead.

For each hypothesis, run the smallest experiment that tests it:

1. Confirm in the profile that the path is hot.
2. Make one change.
3. Run the oracle. A mismatch reverts the change.
4. Race the change against the baseline through bench-guard. Use tango for interleaved A/B timing, gungraun for instruction counts, and Criterion or the crate's own harness for wall-clock time.
5. **Keep** the change when its gain clears the measured noise, every workload stays within tolerance, and the gain pays for its code. Commit it on its own with its numbers, so the gain stays attributable. Otherwise **revert** it.
6. Append the result to the **run log** (`perf-log.md`, unless the user names a place): hypothesis, numbers, and whether it was kept or reverted. A reverted idea stays in the log so later rounds know it was tried.

A kept change that adds `unsafe`, a dependency, threads, or atomics also passes the owner Risk to owner assigns it (Miri, the dependency checks, Loom) before it counts.

Helper agents can widen the hypothesis search. They read code and propose hypotheses. Only the main loop edits the tree and runs benchmarks, because benchmarks run one at a time.

## 3. Converge

Two rounds in a row with low-single-digit cumulative gain (under 3% by default) mean the incremental loop has **converged**.

Then run one **breakthrough** pass, which changes the shape of the computation instead of its details. Options include a better or bespoke algorithm for the real workload, removing work, fusing passes, incremental computation, batching, a data-oriented layout, a strategy chosen by input size, vectorization, parallel decomposition, or dropping generality the contract does not need. A breakthrough is farther from the baseline, so it gets the heavier oracle: the differential harness under a fuzzer or Kani, not only the short random sample.

Stop when the breakthrough pass also converges and no untried hypothesis of a different kind remains. An optional cleanup pass can shrink the code afterward, with every workload held within tolerance.

## 4. Report

Re-run the whole matrix on the final commit against the baseline through bench-guard. Then run the full oracle and the checks owned by each failure class the change touched. Report:

- The metric, the baseline, the final result, the speedup, and the spread, for each workload
- Memory and quality, compared with the baseline
- Each kept change and why it is faster
- The reverted experiments, from the run log
- The bench-guard output, including any waiver and its reason
- The evidence records SKILL.md asks for, and the blind spots

Claim the speedup only for the workloads and bounds you measured.
