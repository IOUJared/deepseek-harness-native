//! The jobs panel's ordering and row text, taken from the reference's own rules: live work in the
//! order it started, finished work newest first, and a live row that states the producer's progress
//! rather than repeating its status (`ui-jobs/src/client/JobListAction.tsx:158-166,203-238`).

use dsh_native_transport::dto::JobView;

/// Whether a job is live: running, or running and being stopped (`JobListAction.tsx:86-96`).
pub fn live(job: &JobView) -> bool {
    job.status == "running" || job.status == "stopping"
}

/// The panel's two sections in the reference's order.
///
/// Live rows sort by their start, oldest first, because the longest-running work is the most
/// interesting. Settled rows sort by their finish, newest first, falling back to the start when a
/// producer reported no finish; equal instants keep start order
/// (`JobListAction.tsx:158-166`). A missing instant sorts after every present one instead of
/// pretending to be the epoch.
pub fn ordered(jobs: &[JobView]) -> (Vec<&JobView>, Vec<&JobView>) {
    let mut live_rows: Vec<&JobView> = jobs.iter().filter(|job| live(job)).collect();
    live_rows.sort_by(|left, right| {
        left.started_at
            .unwrap_or(u64::MAX)
            .cmp(&right.started_at.unwrap_or(u64::MAX))
    });
    let mut settled_rows: Vec<&JobView> = jobs.iter().filter(|job| !live(job)).collect();
    settled_rows.sort_by(|left, right| {
        let settle = |job: &JobView| job.finished_at.or(job.started_at).unwrap_or(0);
        settle(right).cmp(&settle(left)).then_with(|| {
            left.started_at
                .unwrap_or(u64::MAX)
                .cmp(&right.started_at.unwrap_or(u64::MAX))
        })
    });
    (live_rows, settled_rows)
}

/// A live row's second line: its kind and the producer's progress when it states one, else its
/// status. The reference shows `job.progress ?? job.detail` for live rows and never repeats
/// "running" beside them (`JobListAction.tsx:203-238`).
pub fn live_meta(job: &JobView) -> String {
    match job.progress.as_deref().filter(|text| !text.is_empty()) {
        Some(progress) => format!("{} · {progress}", job.kind),
        None => format!("{} · {}", job.kind, job.status),
    }
}

/// A settled row's second line: its kind, its status, and the producer's terminal detail when it
/// recorded one (`exit code: 3`).
pub fn settled_meta(job: &JobView) -> String {
    match job.detail.as_deref().filter(|text| !text.is_empty()) {
        Some(detail) => format!("{} · {} · {detail}", job.kind, job.status),
        None => format!("{} · {}", job.kind, job.status),
    }
}

#[cfg(test)]
mod tests {
    //! The reference's ordering and row text, with the absences the Host may send.
    use super::*;

    fn job(id: &str, status: &str, started: Option<u64>, finished: Option<u64>) -> JobView {
        JobView {
            id: id.into(),
            kind: "bash".into(),
            label: format!("PUBLIC {id}"),
            owner: None,
            status: status.into(),
            progress: None,
            detail: None,
            started_at: started,
            finished_at: finished,
            output: None,
        }
    }

    #[test]
    fn live_rows_come_in_start_order_and_settled_rows_newest_first() {
        let jobs = vec![
            job("settled-old", "completed", Some(100), Some(200)),
            job("live-late", "running", Some(300), None),
            job("settled-new", "failed", Some(150), Some(400)),
            job("live-early", "stopping", Some(50), None),
        ];
        let (live, settled) = ordered(&jobs);
        assert_eq!(
            live.iter().map(|job| job.id.as_str()).collect::<Vec<_>>(),
            ["live-early", "live-late"]
        );
        assert_eq!(
            settled
                .iter()
                .map(|job| job.id.as_str())
                .collect::<Vec<_>>(),
            ["settled-new", "settled-old"]
        );
    }

    #[test]
    fn a_missing_instant_sorts_last_instead_of_reading_as_the_epoch() {
        let jobs = vec![
            job("no-start", "completed", None, None),
            job("dated", "completed", Some(10), Some(20)),
        ];
        let (_, settled) = ordered(&jobs);
        assert_eq!(
            settled
                .iter()
                .map(|job| job.id.as_str())
                .collect::<Vec<_>>(),
            ["dated", "no-start"]
        );
    }

    #[test]
    fn a_live_row_states_progress_when_the_producer_has_it() {
        let mut progress = job("live", "running", Some(1), None);
        assert_eq!(live_meta(&progress), "bash · running");
        progress.progress = Some("3/10".into());
        assert_eq!(live_meta(&progress), "bash · 3/10");
        // An empty progress string is no progress at all.
        progress.progress = Some(String::new());
        assert_eq!(live_meta(&progress), "bash · running");
    }

    #[test]
    fn a_settled_row_states_its_terminal_detail() {
        let mut settled = job("done", "failed", Some(1), Some(2));
        assert_eq!(settled_meta(&settled), "bash · failed");
        settled.detail = Some("exit code: 3".into());
        assert_eq!(settled_meta(&settled), "bash · failed · exit code: 3");
    }
}
