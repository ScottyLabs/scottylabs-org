-- Security re-verification: the append-only audit log lets actor_id go to NULL only while the
-- user it named is being deleted (ON DELETE SET NULL), not whenever someone asks. Before, a plain
-- UPDATE could strip who did anything. Replaces the function only, so the previous release runs.
CREATE OR REPLACE FUNCTION audit_log_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        -- The cascade from DELETE FROM users runs after the user row is gone.
        IF NEW.actor_id IS NULL AND OLD.actor_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM users WHERE id = OLD.actor_id)
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
