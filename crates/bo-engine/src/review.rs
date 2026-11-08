//! Review features over the command bridge: track changes (ECMA-376 Part 1,
//! §17.13.5) and the clock used to date new revisions.

use bo_edit::{CommentInfo, RevisionItem, Session};
use bo_layout::RevisionView;
use serde::Serialize;

/// Review pane state returned with edit results.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewState {
    /// Track changes is on.
    pub track_changes: bool,
    /// Author of new revisions and comments.
    pub author: String,
    /// How tracked changes are displayed.
    pub view: RevisionView,
    /// Revisions of the document.
    pub revisions: Vec<RevisionItem>,
    /// Comments (ECMA-376 Part 1, §17.13.4) with their thread parent.
    pub comments: Vec<CommentInfo>,
}

/// Current review state of a session.
pub fn review_state(s: &Session, view: RevisionView) -> ReviewState {
    ReviewState {
        track_changes: s.track_changes(),
        author: s.author().to_owned(),
        view,
        revisions: s.revisions(),
        comments: s.comments(),
    }
}

/// `ST_DateTime` (ISO 8601, UTC, seconds) for Unix milliseconds
/// (civil-from-days over the proleptic Gregorian calendar).
pub fn iso_date(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

/// System clock in Unix milliseconds; `None` where `std` has no clock (browser WASM,
/// whose host sends `now` with each command instead).
pub fn system_ms() -> Option<i64> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_millis() as i64)
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_date(0), "1970-01-01T00:00:00Z");
        // 2026-11-08T09:30:15Z
        assert_eq!(iso_date(1_794_130_215_000), "2026-11-08T09:30:15Z");
        assert_eq!(iso_date(951_782_400_000), "2000-02-29T00:00:00Z");
    }
}
