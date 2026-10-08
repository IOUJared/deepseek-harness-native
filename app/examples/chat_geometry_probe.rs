//! PUBLIC synthetic text only: standalone geometry tests/microbenchmark, no App/worker/Host.
#[path = "../src/chat_geometry.rs"]
mod chat_geometry;

fn main() {
    let cache = chat_geometry::Cache::default();
    let rows: Vec<_> = (0..4096_u64)
        .map(|key| (key, format!("PUBLIC synthetic preview {key}: native text measurement, not a model response. café · 世界")))
        .collect();
    let cold = std::time::Instant::now();
    let total: f32 = rows
        .iter()
        .map(|(key, text)| {
            cache
                .geometry(
                    *key,
                    if key % 2 == 0 { "You" } else { "Assistant" },
                    text,
                    740.0,
                )
                .height
        })
        .sum();
    let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let warm = std::time::Instant::now();
    for _ in 0..10 {
        cache.retain_keys(rows.iter().map(|(key, _)| *key));
        let again: f32 = rows
            .iter()
            .map(|(key, text)| {
                cache
                    .geometry(
                        *key,
                        if key % 2 == 0 { "You" } else { "Assistant" },
                        text,
                        740.0,
                    )
                    .height
            })
            .sum();
        assert_eq!(total, again);
    }
    let warm_ms = warm.elapsed().as_secs_f64() * 1000.0 / 10.0;
    println!(
        "{}",
        serde_json::json!({"scope":"PUBLIC geometry-only 4096-row release microbenchmark; no GUI/Host or integrated performance claim", "rows":rows.len(), "coldMsIncludingFontInitialization":cold_ms, "warmMeanMs":warm_ms, "heightStable":true})
    );
}
