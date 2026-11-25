-- Indexes for lookups that read approvals and the audit log by subject or by person. Both tables only grow, so without them every lookup reads the whole table.

-- The latest approval of a subject, newer submissions of a subject (a lead's own list, Needs attention) and withdrawing a waiting one.
CREATE INDEX approvals_subject ON approvals (subject_type, subject_id, created_at DESC, id DESC);

-- A lead's own submissions.
CREATE INDEX approvals_submitted_by ON approvals (submitted_by, created_at DESC);

-- The latest entry about a subject, which autosaves update instead of adding another.
CREATE INDEX audit_log_subject ON audit_log (subject_type, subject_id, created_at DESC, id DESC);

-- Recent activity: a person's own actions.
CREATE INDEX audit_log_actor ON audit_log (actor_id, created_at DESC, id DESC);
