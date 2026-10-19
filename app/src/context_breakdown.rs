//! Where the context window went: the reference's `contextBreakdown` projection, read into the three
//! rows its panel draws (`ContextMeter.tsx:29-33,57-58,168-176`, `locales.ts:445-448`).
//!
//! Native never read this projection: it drew the occupancy line and a policy disclosure, so a user
//! could see how full the window was but not what filled it. The numbers here are the Host's own
//! heuristic split, so every one of them is stated with the reference's `~`.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The three rows, in the reference's bar-segment order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Breakdown {
    pub system: u64,
    pub tools: u64,
    pub messages: u64,
}

impl Breakdown {
    pub fn total(&self) -> u64 {
        self.system
            .saturating_add(self.tools)
            .saturating_add(self.messages)
    }
    /// The bar's segments: one width per row, proportional to that row's share of the heuristic total
    /// and scaled to the overall occupancy, in the reference's order
    /// (`ContextMeter.tsx:97-106`). A zero part is dropped rather than drawn as a hairline; a zero
    /// total leaves one neutral segment filling the percent.
    pub fn segments(&self, percent: u64) -> [Option<f32>; 3] {
        let total = self.total();
        if total == 0 {
            return [Some(percent as f32), None, None];
        }
        let width = |tokens: u64| -> Option<f32> {
            let share = tokens as f64 / total as f64;
            let width = percent as f64 * share;
            (width > 0.0).then_some(width as f32)
        };
        [width(self.system), width(self.tools), width(self.messages)]
    }
    /// The rows the panel lists, labelled as the reference labels them.
    pub fn rows(&self) -> [(&'static str, u64); 3] {
        [
            ("System prompt", self.system),
            ("Tool definitions", self.tools),
            ("Messages", self.messages),
        ]
    }
}

/// Read the projection. Absent, malformed, or entirely zero is `None`: the reference draws no rows at
/// all in that case rather than three zeroes.
pub fn read(projections: &BTreeMap<String, Value>) -> Option<Breakdown> {
    let object: &Map<String, Value> = projections.get("contextBreakdown")?.as_object()?;
    let field = |name: &str| object.get(name).and_then(Value::as_u64).unwrap_or(0);
    let breakdown = Breakdown {
        system: field("systemTokens"),
        tools: field("toolsTokens"),
        messages: field("messageTokens"),
    };
    (breakdown.total() > 0).then_some(breakdown)
}

/// The reading the panel's headline states: the projected (or pressure) count and the window, read
/// from the same projection the occupancy line uses (`ContextMeter.tsx:62,168-176`).
pub fn reading(projections: &BTreeMap<String, Value>) -> Option<(u64, u64)> {
    let object = projections.get("contextPressure")?.as_object()?;
    let window = object.get("contextWindow").and_then(Value::as_u64)?;
    if window == 0 {
        return None;
    }
    let used = object
        .get("projectedTokens")
        .and_then(Value::as_u64)
        .or_else(|| object.get("pressureTokens").and_then(Value::as_u64))?;
    Some((used, window))
}

/// Integer percentage of the window in use, `0` without a reading.
pub fn reading_percent(used: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    (u128::from(used) * 100 / u128::from(window)) as u64
}

/// The panel's compact token form: `12K`, `1.2M`, `999` — the reference's `number.thousand` /
/// `number.million` templates with one decimal below a hundred (`ContextMeter.tsx:38-46`).
pub fn format_tokens(value: u64) -> String {
    let scaled = |candidate: f64| {
        if candidate >= 100.0 {
            format!("{candidate:.0}")
        } else {
            let rounded = (candidate * 10.0).round() / 10.0;
            format!("{rounded}")
        }
    };
    if value < 1_000 {
        value.to_string()
    } else if value < 1_000_000 {
        format!("{}K", scaled(value as f64 / 1_000.0))
    } else {
        format!("{}M", scaled(value as f64 / 1_000_000.0))
    }
}

#[cfg(test)]
mod tests {
    //! The projection reader and the panel's number form.
    use super::*;
    use serde_json::json;

    #[test]
    fn the_breakdown_reads_three_shares_and_refuses_nothing_to_show() {
        let projections = BTreeMap::from([(
            "contextBreakdown".to_owned(),
            json!({"systemTokens": 12_400, "toolsTokens": 3_600, "messageTokens": 28_000}),
        )]);
        let breakdown = read(&projections).expect("a breakdown");
        assert_eq!(breakdown.total(), 44_000);
        assert_eq!(
            breakdown.rows(),
            [
                ("System prompt", 12_400),
                ("Tool definitions", 3_600),
                ("Messages", 28_000)
            ]
        );
        // Absent, malformed and all-zero are all "no rows".
        assert!(read(&BTreeMap::new()).is_none());
        assert!(
            read(&BTreeMap::from([(
                "contextBreakdown".to_owned(),
                json!("PUBLIC not an object")
            )]))
            .is_none()
        );
        assert!(
            read(&BTreeMap::from([(
                "contextBreakdown".to_owned(),
                json!({"systemTokens": 0, "toolsTokens": 0, "messageTokens": 0})
            )]))
            .is_none()
        );
    }

    #[test]
    fn the_bar_segments_share_the_occupancy_in_the_references_order() {
        let breakdown = Breakdown {
            system: 12_400,
            tools: 3_600,
            messages: 28_000,
        };
        let segments = breakdown.segments(84);
        // 12400/44000, 3600/44000 and 28000/44000 of 84%.
        assert!((segments[0].unwrap() - 23.67).abs() < 0.1);
        assert!((segments[1].unwrap() - 6.87).abs() < 0.1);
        assert!((segments[2].unwrap() - 53.45).abs() < 0.1);
        // A zero part is dropped, not drawn as a hairline.
        let system_only = Breakdown {
            system: 1_000,
            tools: 0,
            messages: 0,
        };
        assert_eq!(system_only.segments(84), [Some(84.0), None, None]);
        // No breakdown at all: one neutral segment states the occupancy.
        let empty = Breakdown {
            system: 0,
            tools: 0,
            messages: 0,
        };
        assert_eq!(empty.segments(42), [Some(42.0), None, None]);
    }

    #[test]
    fn the_headline_reading_matches_the_occupancy_line() {
        let projections = BTreeMap::from([(
            "contextPressure".to_owned(),
            json!({"pressureTokens": 60_000, "projectedTokens": 108_000, "contextWindow": 128_000}),
        )]);
        assert_eq!(reading(&projections), Some((108_000, 128_000)));
        assert_eq!(reading_percent(108_000, 128_000), 84);
        // Pressure alone is the reading when nothing is projected.
        let pressure_only = BTreeMap::from([(
            "contextPressure".to_owned(),
            json!({"pressureTokens": 64_000, "contextWindow": 128_000}),
        )]);
        assert_eq!(reading(&pressure_only), Some((64_000, 128_000)));
        // No window, or a zero window, is no reading.
        assert!(
            reading(&BTreeMap::from([(
                "contextPressure".to_owned(),
                json!({"pressureTokens": 1, "contextWindow": 0})
            )]))
            .is_none()
        );
    }

    #[test]
    fn token_counts_are_compact_with_the_references_rounding() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_000), "1K");
        assert_eq!(format_tokens(12_400), "12.4K");
        assert_eq!(format_tokens(123_400), "123K");
        assert_eq!(format_tokens(1_240_000), "1.2M");
        assert_eq!(format_tokens(12_400_000), "12.4M");
    }
}
