-- Security re-verification of sign-in. Additive only, so the previous release still runs.

-- Admins removed or lowered on the People page whom the Keycloak admin group would otherwise make
-- admins again at their next sign-in or session check. The row goes when a token shows they left
-- the group, or when an admin gives them the admin role again on the People page.
CREATE TABLE admin_group_opt_outs (
    user_id uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    created_by uuid REFERENCES users (id) ON DELETE SET NULL
);

-- When the person first signed in here with Keycloak. Kept when an admin resets the link, so dev
-- sign-in on a preview never acts as someone who has used their real account here.
ALTER TABLE users ADD COLUMN keycloak_linked_at timestamptz;
UPDATE users SET keycloak_linked_at = now()
WHERE keycloak_sub IS NOT NULL
   OR id::text IN (SELECT subject_id FROM audit_log WHERE action IN ('auth.link', 'auth.link_reset'));

-- The CSRF token of sessions made by releases that stored it (and sent it to the browser from
-- /api/me), so tabs opened before this release keep saving. Copied only for sessions from before
-- 0701, when the stored value stopped being a working token; they end within seven days.
ALTER TABLE sessions ADD COLUMN legacy_csrf_token text;
UPDATE sessions SET legacy_csrf_token = csrf_token
WHERE created_at < coalesce((SELECT installed_on FROM _sqlx_migrations WHERE version = 701), now());
