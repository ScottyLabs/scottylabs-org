//! Image upload processing and storage in S3 or Postgres. Platform track.

use sqlx::PgPool;

use crate::config::S3Settings;

pub struct MediaStore {
    pub s3: Option<S3Settings>,
    pub db: PgPool,
    pub http: reqwest::Client,
}

impl MediaStore {
    pub fn new(s3: Option<S3Settings>, db: PgPool, http: reqwest::Client) -> Self {
        Self { s3, db, http }
    }
}
