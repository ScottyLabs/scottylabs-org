-- Security audit fixes: an append-only audit log with details, retention, and what admins need to
-- review roles that governance does not list. Additive only, so the previous release still runs.

-- Before and after values for role changes, settings, slug changes, publishes, approvals and
-- deletions, written by audit::record_details. Null for entries that have none.
ALTER TABLE audit_log ADD COLUMN details jsonb;

-- The audit log only grows. Rows are never edited and are deleted only by expire_audit_log(),
-- once they are more than two years old. A user's deletion may still clear actor_id
-- (ON DELETE SET NULL), so a person can be erased while the log keeps what happened.
-- Other UPDATEs are skipped rather than refused: the previous release refreshed its latest
-- "Edited <project>" entry in place, and when no row changes it adds a new entry instead.
CREATE FUNCTION audit_log_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.actor_id IS NULL AND OLD.actor_id IS NOT NULL
           AND (NEW.id, NEW.action, NEW.subject_type, NEW.subject_id, NEW.committee, NEW.summary,
                NEW.created_at, NEW.details)
               IS NOT DISTINCT FROM
               (OLD.id, OLD.action, OLD.subject_type, OLD.subject_id, OLD.committee, OLD.summary,
                OLD.created_at, OLD.details) THEN
            RETURN NEW;
        END IF;
        RAISE WARNING 'audit_log is append-only; an UPDATE of entry % was skipped', OLD.id;
        RETURN NULL;
    END IF;
    IF TG_OP = 'DELETE'
       AND current_setting('site.expiring_audit_log', true) = 'on'
       AND OLD.created_at < now() - interval '2 years' THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'audit_log is append-only; entries older than two years are removed by expire_audit_log()'
        USING ERRCODE = 'insufficient_privilege';
END
$$;

CREATE TRIGGER audit_log_append_only
    BEFORE UPDATE OR DELETE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_append_only();

CREATE TRIGGER audit_log_no_truncate
    BEFORE TRUNCATE ON audit_log
    FOR EACH STATEMENT EXECUTE FUNCTION audit_log_append_only();

-- The one way entries leave the log: everything older than two years, the retention in
-- docs/implementation/contract.md. Returns how many were removed.
CREATE FUNCTION expire_audit_log() RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
    removed bigint;
BEGIN
    PERFORM set_config('site.expiring_audit_log', 'on', true);
    DELETE FROM audit_log WHERE created_at < now() - interval '2 years';
    GET DIAGNOSTICS removed = ROW_COUNT;
    PERFORM set_config('site.expiring_audit_log', 'off', true);
    RETURN removed;
END
$$;

-- When a role was last changed and by whom, so admins can review roles the People page gave.
ALTER TABLE role_assignments
    ADD COLUMN updated_at timestamptz,
    ADD COLUMN updated_by uuid REFERENCES users (id) ON DELETE SET NULL;
UPDATE role_assignments SET updated_at = created_at;
ALTER TABLE role_assignments
    ALTER COLUMN updated_at SET DEFAULT now(),
    ALTER COLUMN updated_at SET NOT NULL;

-- Which role the People page took away: 'director' keeps the sync from giving the person any role
-- in the committee, 'lead' only from making them a lead again, so removing a lead never blocks
-- governance from appointing them director. Rows from before this migration keep blocking both.
ALTER TABLE governance_opt_outs
    ADD COLUMN role text NOT NULL DEFAULT 'director' CHECK (role IN ('director', 'lead'));

-- The director governance listed for each committee at the last sync, beside governance_heads.
CREATE TABLE governance_directors (
    committee text PRIMARY KEY REFERENCES committees (slug),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE
);

-- The last governance sync, for the People page: background runs log only counts.
CREATE TABLE governance_sync_status (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    last_attempt_at timestamptz,
    last_success_at timestamptz,
    -- Why the last attempt failed, as shown to admins; null after a success.
    last_error text,
    -- The messages of the last successful run, as GovernanceSyncResponse.messages.
    messages jsonb NOT NULL DEFAULT '[]'
);

-- When an upload stopped being used by any draft, published version or open submission. The
-- cleanup task sets it, clears it when the image is used again, and deletes the image seven days
-- after it was set.
ALTER TABLE media ADD COLUMN unreferenced_since timestamptz;

-- Uploads per person per day, for the upload quota.
CREATE INDEX media_uploaded_by ON media (uploaded_by, created_at DESC);
-- Expired sessions, for the cleanup task.
CREATE INDEX sessions_last_seen ON sessions (last_seen_at);
