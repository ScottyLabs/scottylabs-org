-- Seeds that run once per database, rather than whenever their table happens to be empty, so content people delete stays deleted.
CREATE TABLE seed_markers (
    name text PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- A database that already has, or had, announcements was seeded before this table existed.
INSERT INTO seed_markers (name)
SELECT 'ticker'
WHERE EXISTS (SELECT 1 FROM announcements)
   OR EXISTS (SELECT 1 FROM audit_log WHERE subject_type = 'announcement');
