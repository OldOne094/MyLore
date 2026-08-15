//! MISSION-021: database benchmarks.
//!
//! Measures the persistence paths that matter for UX:
//!   - `insert/repo_create_*`   — single-add through the repository (`media::create`,
//!     its own transaction + FTS triggers), the manual-add path.
//!   - `insert/bulk_*`          — bulk import timing: raw multi-row insert in one
//!     transaction (the MISSION-067 import path) vs. a naive per-row repo loop.
//!   - `search/fts_*`           — FTS5 `media::search` latency on 10k/50k/100k rows.
//!
//! Run with `cargo bench --bench database` (release). Each insert iteration runs
//! against a fresh in-memory migrated database so numbers are independent of
//! prior samples.

use std::str::FromStr;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tokio::runtime::Runtime;

use mylore_lib::infrastructure::{db, repositories::media};

/// A fresh in-memory database with the full schema + seeds migrated.
fn fresh_pool(rt: &Runtime) -> SqlitePool {
    rt.block_on(async {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("valid memory uri")
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .pragma("recursive_triggers", "ON");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("open in-memory database");
        db::migrate(&pool).await.expect("migrate");
        pool
    })
}

/// Insert `n` media rows in a single transaction via raw SQL — the fast bulk
/// path (FTS refresh happens per-row through triggers inside the tx).
fn bulk_insert_raw(rt: &Runtime, pool: &SqlitePool, n: usize) {
    rt.block_on(async {
        let mut tx = pool.begin().await.expect("begin");
        for i in 0..n {
            sqlx::query(
                "INSERT INTO media
                   (id, content_type, format, title_main, synopsis, pub_status, release_year,
                    created_at, updated_at)
                 VALUES (?, 'novel', 'light_novel', ?, 'A benchmark title.', 'ongoing', 2025,
                    '2026-01-01', '2026-01-01')",
            )
            .bind(format!("m-bench-{i}"))
            .bind(format!("Chronicle {i}"))
            .execute(&mut *tx)
            .await
            .expect("bulk insert");
        }
        tx.commit().await.expect("commit");
    });
}

/// Insert `n` media through the repository (one transaction per record).
fn bulk_insert_repo_loop(rt: &Runtime, pool: &SqlitePool, n: usize) {
    rt.block_on(async {
        for i in 0..n {
            media::create(
                pool,
                &media::MediaRecord {
                    id: format!("m-repo-{i}"),
                    content_type: "novel".into(),
                    format: Some("light_novel".into()),
                    title_main: format!("Chronicle {i}"),
                    title_original: None,
                    synopsis: Some("A benchmark title.".into()),
                    pub_status: "ongoing".into(),
                    start_date: None,
                    end_date: None,
                    release_year: Some(2025),
                    language: Some("ja".into()),
                    country: None,
                    content_rating: None,
                    pages: None,
                    duration_min: None,
                    ep_count: None,
                    ch_count: None,
                    cover_asset_id: None,
                    banner_asset_id: None,
                    provider: Some("anilist".into()),
                    provider_url: None,
                    metadata_refreshed_at: None,
                    created_at: "2026-01-01".into(),
                    updated_at: "2026-01-01".into(),
                    alt_titles: Vec::new(),
                    people: Vec::new(),
                    genres: Vec::new(),
                    tags: Vec::new(),
                    external_ids: Vec::new(),
                    relations: Vec::new(),
                },
            )
            .await
            .expect("repo create");
        }
    });
}

/// Build a database seeded with `n` media where roughly 1% of titles carry the
/// search term `Nexus`, so the query returns a realistic, bounded result set.
fn seeded_pool(rt: &Runtime, n: usize) -> SqlitePool {
    let pool = fresh_pool(rt);
    rt.block_on(async {
        let mut tx = pool.begin().await.expect("begin");
        for i in 0..n {
            let (title, synopsis) = if i % 100 == 0 {
                (
                    format!("Nexus Signal {i}"),
                    format!("The singularity awakens in chapter {i}."),
                )
            } else {
                (
                    format!("The Quiet Journey of the Long Road {i}"),
                    String::new(),
                )
            };
            sqlx::query(
                "INSERT INTO media
                   (id, content_type, format, title_main, synopsis, pub_status, release_year,
                    created_at, updated_at)
                 VALUES (?, 'novel', 'light_novel', ?, ?, 'ongoing', 2025,
                    '2026-01-01', '2026-01-01')",
            )
            .bind(format!("m-{i}"))
            .bind(title)
            .bind(synopsis)
            .execute(&mut *tx)
            .await
            .expect("seed insert");
        }
        tx.commit().await.expect("commit");
    });
    pool
}

fn count_media(pool: &SqlitePool) -> i64 {
    let rt = Runtime::new().expect("runtime");
    rt.block_on(async {
        let row = sqlx::query("SELECT COUNT(*) FROM media")
            .fetch_one(pool)
            .await
            .unwrap();
        row.get::<i64, _>(0)
    })
}

fn bench_insert(c: &mut Criterion) {
    let rt = Runtime::new().expect("runtime");
    let mut group = c.benchmark_group("insert");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(4));

    for n in [1_000usize, 10_000] {
        group.bench_with_input(BenchmarkId::new("repo_create", n), &n, |b, &n| {
            b.iter_batched(
                || fresh_pool(&rt),
                |pool| bulk_insert_repo_loop(&rt, &pool, n),
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("bulk_raw_one_tx", n), &n, |b, &n| {
            b.iter_batched(
                || fresh_pool(&rt),
                |pool| bulk_insert_raw(&rt, &pool, n),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let rt = Runtime::new().expect("runtime");
    let mut group = c.benchmark_group("search");
    group.sample_size(30);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(4));

    for n in [10_000usize, 50_000, 100_000] {
        let pool = seeded_pool(&rt, n);
        assert_eq!(count_media(&pool), n as i64);

        group.bench_with_input(BenchmarkId::new("fts_selective", n), &n, |b, _| {
            b.iter(|| rt.block_on(media::search(&pool, "nexus")));
        });
        group.bench_with_input(BenchmarkId::new("fts_no_match", n), &n, |b, _| {
            b.iter(|| rt.block_on(media::search(&pool, "zzzzyggg")));
        });
        rt.block_on(pool.close());
    }
    group.finish();
}

fn bench_bulk_import(c: &mut Criterion) {
    let rt = Runtime::new().expect("runtime");
    let mut group = c.benchmark_group("bulk_import");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(4));

    group.bench_with_input(
        BenchmarkId::new("raw_one_tx", 10_000usize),
        &10_000,
        |b, &n| {
            b.iter_batched(
                || fresh_pool(&rt),
                |pool| bulk_insert_raw(&rt, &pool, n),
                BatchSize::SmallInput,
            );
        },
    );
    group.bench_with_input(
        BenchmarkId::new("repo_loop", 10_000usize),
        &10_000,
        |b, &n| {
            b.iter_batched(
                || fresh_pool(&rt),
                |pool| bulk_insert_repo_loop(&rt, &pool, n),
                BatchSize::SmallInput,
            );
        },
    );
    group.finish();
}

criterion_group!(
    name = database;
    config = Criterion::default().warm_up_time(Duration::from_millis(300));
    targets = bench_insert, bench_search, bench_bulk_import
);
criterion_main!(database);
