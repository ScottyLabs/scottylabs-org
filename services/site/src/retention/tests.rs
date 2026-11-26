//! The hourly cleanup against a real database: what each rule removes, what it keeps, and that running it again changes nothing.

use std::time::Duration;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::*;
use crate::media::MediaStore;
use crate::models::{Committee, Role};
use crate::testing;

fn store(db: &PgPool) -> MediaStore {
    MediaStore::new(None, db.clone(), reqwest::Client::new()).unwrap()
}

/// An upload row with one stored variant, made `days_ago`.
async fn media(db: &PgPool, days_ago: i32) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO media (id, content_type, width, height, variants, bytes, storage, created_at)
         VALUES ($1, 'image/jpeg', 10, 10, '{10}', 3, 'database', now() - make_interval(days => $2))",
    )
    .bind(id)
    .bind(days_ago)
    .execute(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO media_blobs (media_id, width, data) VALUES ($1, 10, '\\x010203')")
        .bind(id)
        .execute(db)
        .await
        .unwrap();
    id
}

async fn exists(db: &PgPool, table: &str, id: Uuid) -> bool {
    sqlx::query_scalar(&format!(
        "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1)"
    ))
    .bind(id)
    .fetch_one(db)
    .await
    .unwrap()
}

/// Marks an upload as unused for `days`.
async fn unused_for(db: &PgPool, id: Uuid, days: i32) {
    sqlx::query(
        "UPDATE media SET unreferenced_since = now() - make_interval(days => $2) WHERE id = $1",
    )
    .bind(id)
    .bind(days)
    .execute(db)
    .await
    .unwrap();
}

/// An announcement that left the ticker `days_ago`, in `state`, with an approval.
async fn announcement(db: &PgPool, state: &str, days_ago: i32) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO announcements (id, kind, committee, message, state, expires_at)
         VALUES ($1, 'event', 'events', 'Old news', $2, now() - make_interval(days => $3))",
    )
    .bind(id)
    .bind(state)
    .bind(days_ago)
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO approvals (id, subject_type, subject_id, committee, snapshot, status)
         VALUES ($1, 'announcement', $2, 'events', '{}', 'approved')",
    )
    .bind(Uuid::now_v7())
    .bind(id)
    .execute(db)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_attempts_and_old_announcements_go(db: PgPool) {
    testing::ensure_committees(&db).await;
    let fresh = testing::user(&db, "fresh", &[(Role::Lead, Some(Committee::Events))]).await;
    let idle = testing::user(&db, "idle", &[]).await;
    sqlx::query(
        "UPDATE sessions SET last_seen_at = now() - interval '13 hours' WHERE user_id = $1",
    )
    .bind(idle.id)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO auth_attempts (csrf, pkce_verifier, nonce, return_to, created_at)
         VALUES ('old', 'v', 'n', '/portal/', now() - interval '2 hours'),
                ('new', 'v', 'n', '/portal/', now())",
    )
    .execute(&db)
    .await
    .unwrap();
    let ancient = announcement(&db, "live", 91).await;
    let ended = announcement(&db, "ended", 120).await;
    let recent = announcement(&db, "live", 60).await;
    let never_approved = announcement(&db, "sent_back", 95).await;

    let report = run(&db, &store(&db)).await.unwrap().unwrap();
    assert_eq!(report.sessions, 1);
    assert_eq!(report.sign_in_attempts, 1);
    assert_eq!(report.announcements, 3);
    assert_eq!(report.approvals, 3);
    for gone in [ancient, ended, never_approved] {
        assert!(!exists(&db, "announcements", gone).await);
    }
    assert!(exists(&db, "announcements", recent).await);
    let approvals: i64 = sqlx::query_scalar("SELECT count(*) FROM approvals")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(approvals, 1);
    let sessions: Vec<Uuid> = sqlx::query_scalar("SELECT user_id FROM sessions")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(sessions, [fresh.id]);

    // Running again finds nothing more.
    let again = run(&db, &store(&db)).await.unwrap().unwrap();
    assert_eq!(again, Report::default());
}

#[sqlx::test(migrations = "./migrations")]
async fn unused_uploads_go_after_a_week(db: PgPool) {
    testing::ensure_committees(&db).await;
    let used = media(&db, 30).await;
    let unused = media(&db, 30).await;
    let recently_unused = media(&db, 30).await;
    let submitted = media(&db, 30).await;
    let nova = testing::project(&db, Committee::Events, "nova").await;
    sqlx::query("UPDATE projects SET draft = jsonb_set(draft, '{media}', $2) WHERE id = $1")
        .bind(nova)
        .bind(json!([{ "src": format!("/media/{used}"), "alt": "x" }]))
        .execute(&db)
        .await
        .unwrap();
    // A submission waiting for approval keeps its photos, however the id is written.
    sqlx::query(
        "INSERT INTO approvals (id, subject_type, subject_id, committee, snapshot, status)
         VALUES ($1, 'project', $2, 'events', $3, 'waiting')",
    )
    .bind(Uuid::now_v7())
    .bind(nova)
    .bind(
        json!({ "media": [{ "src": format!("/media/{}", submitted.to_string().to_uppercase()) }] }),
    )
    .execute(&db)
    .await
    .unwrap();

    // The first run notes what nothing uses; nothing is deleted before a week has passed.
    let report = run(&db, &store(&db)).await.unwrap().unwrap();
    assert_eq!((report.media_unused, report.media_deleted), (2, 0));
    assert!(exists(&db, "media", unused).await);

    // After a week unused, an upload goes, files and all.
    unused_for(&db, unused, 8).await;
    unused_for(&db, recently_unused, 6).await;
    let report = run(&db, &store(&db)).await.unwrap().unwrap();
    assert_eq!(report.media_deleted, 1, "{report:?}");
    assert!(!exists(&db, "media", unused).await);
    let blobs: i64 = sqlx::query_scalar("SELECT count(*) FROM media_blobs WHERE media_id = $1")
        .bind(unused)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(blobs, 0);
    for kept in [used, submitted, recently_unused] {
        assert!(exists(&db, "media", kept).await);
    }
    let expired: (Option<Uuid>, serde_json::Value) =
        sqlx::query_as("SELECT actor_id, details FROM audit_log WHERE action = 'media.expire'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(expired.0, None);
    assert_eq!(expired.1["before"]["id"], unused.to_string());

    // Once a draft uses an image again, its clock stops.
    sqlx::query("UPDATE projects SET draft = jsonb_set(draft, '{media}', $2) WHERE id = $1")
        .bind(nova)
        .bind(json!([{ "src": format!("/media/{recently_unused}"), "alt": "x" }]))
        .execute(&db)
        .await
        .unwrap();
    let report = run(&db, &store(&db)).await.unwrap().unwrap();
    assert_eq!(report.media_used_again, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_image_used_again_is_kept(db: PgPool) {
    testing::ensure_committees(&db).await;
    let id = media(&db, 30).await;
    run(&db, &store(&db)).await.unwrap();
    unused_for(&db, id, 8).await;
    // An editor puts it on a draft after the run listed it as due.
    let nova = testing::project(&db, Committee::Events, "nova").await;
    sqlx::query("UPDATE projects SET draft = jsonb_set(draft, '{media}', $2) WHERE id = $1")
        .bind(nova)
        .bind(json!([{ "src": format!("/media/{id}"), "alt": "x" }]))
        .execute(&db)
        .await
        .unwrap();
    assert!(!delete_unused(&db, &store(&db), id).await.unwrap());
    let since: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT unreferenced_since FROM media WHERE id = $1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(since.is_none());
}

/// Uploads kept in S3 lose their objects too. Encoding the image takes seconds in a debug build, so the test gets a database of its own (see `testing::IsolatedDb`).
#[tokio::test]
async fn unused_uploads_in_s3_lose_their_objects() {
    let isolated = testing::IsolatedDb::new().await;
    let db = &isolated.pool;
    testing::ensure_committees(db).await;
    let store = MediaStore::new(
        Some(crate::media::tests::fake_s3(Duration::ZERO).await),
        db.clone(),
        reqwest::Client::new(),
    )
    .unwrap();
    let owner = testing::user(db, "s3owner", &[]).await;
    let processed =
        crate::media::process::process(&crate::media::process::tests::jpeg(600, 300)).unwrap();
    let id = Uuid::now_v7();
    store.store_objects(id, &processed).await.unwrap();
    let mut tx = db.begin().await.unwrap();
    store
        .insert(&mut tx, id, &processed, owner.id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(store.open(id, None).await.unwrap().is_some());

    run(db, &store).await.unwrap();
    unused_for(db, id, 8).await;
    let report = run(db, &store).await.unwrap().unwrap();
    assert_eq!(report.media_deleted, 1);
    assert!(store.open(id, None).await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_entries_leave_only_after_two_years(db: PgPool) {
    testing::ensure_committees(&db).await;
    for days in [800, 700] {
        sqlx::query(
            "INSERT INTO audit_log (id, action, subject_type, summary, created_at)
             VALUES ($1, 'test.old', 'person', 'Old entry', now() - make_interval(days => $2))",
        )
        .bind(Uuid::now_v7())
        .bind(days)
        .execute(&db)
        .await
        .unwrap();
    }
    // Only expire_audit_log() may delete, and only entries past retention.
    assert!(
        sqlx::query("DELETE FROM audit_log")
            .execute(&db)
            .await
            .is_err()
    );
    let report = run(&db, &store(&db)).await.unwrap().unwrap();
    assert_eq!(report.audit_entries, 1);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(left, 1);
    // Setting the switch by hand does not unlock younger entries.
    let mut tx = db.begin().await.unwrap();
    sqlx::query("SELECT set_config('site.expiring_audit_log', 'on', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM audit_log")
            .execute(&mut *tx)
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn erasing_a_person_keeps_their_entries_without_them(db: PgPool) {
    testing::ensure_committees(&db).await;
    let gone = testing::user(&db, "leaver", &[]).await;
    sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, subject_type, summary)
         VALUES ($1, $2, 'test.action', 'person', 'Did something')",
    )
    .bind(Uuid::now_v7())
    .bind(gone.id)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(gone.id)
        .execute(&db)
        .await
        .unwrap();
    let actor: Option<Uuid> = sqlx::query_scalar("SELECT actor_id FROM audit_log")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(actor, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_second_replica_skips_the_run(db: PgPool) {
    testing::ensure_committees(&db).await;
    let mut other = db.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(CLEANUP_LOCK)
        .execute(&mut *other)
        .await
        .unwrap();
    assert!(run(&db, &store(&db)).await.unwrap().is_none());
    other.rollback().await.unwrap();
    assert!(run(&db, &store(&db)).await.unwrap().is_some());
}
