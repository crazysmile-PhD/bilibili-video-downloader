//! Focused CPU benchmark; deliberately excludes HTTP, disk and GUI startup.
#[path = "../src/bili_client/response.rs"]
mod response;

use std::{hint::black_box, time::Instant};

use eyre::WrapErr;
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Item {
    id: u64,
    title: String,
    duration: u64,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Payload {
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct LegacyResponse {
    code: i64,
    #[serde(default, alias = "message")]
    msg: String,
    #[serde(alias = "result")]
    data: Option<serde_json::Value>,
}

fn legacy_decode(body: &str) -> eyre::Result<Payload> {
    // Match the former endpoint implementation, including eager error context.
    let envelope: LegacyResponse =
        serde_json::from_str(body).wrap_err(format!("将body解析为BiliResp失败: {body}"))?;
    eyre::ensure!(envelope.code == 0, "API error: {}", envelope.msg);
    let data = envelope.data.ok_or_else(|| eyre::eyre!("missing data"))?;
    let text = data.to_string();
    serde_json::from_str(&text).wrap_err(format!("将data解析为Payload失败: {text}"))
}

fn time(
    body: &str,
    iterations: usize,
    decode: fn(&str) -> eyre::Result<Payload>,
) -> eyre::Result<f64> {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(decode(black_box(body))?);
    }
    Ok(start.elapsed().as_secs_f64() * 1_000_000.0 / iterations as f64)
}

fn optimized_decode(body: &str) -> eyre::Result<Payload> {
    response::decode_api_response(200, body)
}

fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    (values[(values.len() - 1) / 2] + values[values.len() / 2]) / 2.0
}

fn main() -> eyre::Result<()> {
    let mut cases = Vec::new();
    for size in [20, 200, 2000] {
        let payload = Payload {
            items: (0..size)
                .map(|id| Item {
                    id,
                    title: format!("视频标题 {id} & Unicode 👋"),
                    duration: id * 10,
                })
                .collect(),
        };
        let body = serde_json::to_string(&serde_json::json!({"code": 0, "data": payload}))?;
        eyre::ensure!(
            legacy_decode(&body)? == optimized_decode(&body)?,
            "decoded payloads differ"
        );
        for _ in 0..20 {
            black_box(legacy_decode(&body)?);
            black_box(optimized_decode(&body)?);
        }
        let iterations = (4000 / size as usize).max(10);
        let mut legacy = Vec::new();
        let mut optimized = Vec::new();
        for sample in 0..30 {
            // Alternate order to reduce warm-cache/thermal order bias.
            if sample % 2 == 0 {
                legacy.push(time(&body, iterations, legacy_decode)?);
                optimized.push(time(&body, iterations, optimized_decode)?);
            } else {
                optimized.push(time(&body, iterations, optimized_decode)?);
                legacy.push(time(&body, iterations, legacy_decode)?);
            }
        }
        let ratios: Vec<f64> = legacy
            .iter()
            .zip(&optimized)
            .map(|(old, new)| old / new)
            .collect();
        let mut rng = StdRng::seed_from_u64(20261119);
        let mut bootstrap = Vec::new();
        for _ in 0..2000 {
            let sample: Vec<f64> = (0..ratios.len())
                .map(|_| ratios[rng.random_range(0..ratios.len())])
                .collect();
            bootstrap.push(median(&sample));
        }
        bootstrap.sort_by(f64::total_cmp);
        cases.push(serde_json::json!({
            "items": size, "bytes": body.len(), "iterations_per_sample": iterations,
            "legacy_us": legacy, "optimized_us": optimized,
            "legacy_median_us": median(&legacy), "optimized_median_us": median(&optimized),
            "paired_speedup_median": median(&ratios),
            "paired_speedup_bootstrap_95_percent": [bootstrap[50], bootstrap[1949]],
        }));
    }
    let output = serde_json::json!({
        "scope": "CPU-only JSON decoding microbenchmark; not end-to-end download throughput",
        "profile": "cargo bench; opt-level=3; lto=false; codegen-units=16",
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "samples": 30, "warmup_pairs": 20, "cases": cases,
    });
    let path = std::env::var("BENCH_OUTPUT").unwrap_or_else(|_| "api-decode-benchmark.json".into());
    let file = std::fs::File::create(&path)?;
    serde_json::to_writer_pretty(file, &output)?;
    println!("Benchmark samples written to {path}");
    Ok(())
}
