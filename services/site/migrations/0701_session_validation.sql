-- Sessions are checked against Keycloak while they last, and dev sessions remember the key they
-- were made with. See docs/implementation/contract.md, "Sessions". Additive only, so the previous
-- release still starts against this schema.

ALTER TABLE sessions
    -- Keycloak's refresh token and latest ID token for sessions that signed in through it. Kept on
    -- the server only, never sent to a browser. Using the refresh token also needs the client
    -- secret.
    ADD COLUMN refresh_token text,
    ADD COLUMN id_token text,
    -- When Keycloak last confirmed the session: at sign-in, then at each refresh. Null for dev
    -- sessions and for sessions from before this migration, which count from created_at.
    ADD COLUMN validated_at timestamptz,
    -- No refresh is started before this. Set while one runs, so only one runs at a time, and a
    -- few minutes ahead after Keycloak could not be reached.
    ADD COLUMN validation_retry_at timestamptz,
    -- Dev sessions on previews: a fingerprint of the key they were made with, so changing the key
    -- ends them.
    ADD COLUMN dev_key_fingerprint bytea;
