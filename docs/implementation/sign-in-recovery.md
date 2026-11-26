# Sign-in recovery

What to do when people cannot sign in to the portal, or when a session must end now. The rules behind each step are in `contract.md` (Auth). Run SQL on the site's own database, as the service's database user, and put the Andrew ID in lowercase.

## Someone gets "linked to a different ScottyLabs account"

The sign-in page says `error=linked_elsewhere`, and the log says `refused a Keycloak sign-in: this Andrew ID is linked to a different Keycloak account`. Keycloak gave the person a new account (a new `sub`), for example after their account was deleted and created again. The portal refuses on purpose: otherwise anyone who got a Keycloak account with that username would take over the person's roles.

1. Check with the person that they really are the owner of that Andrew ID, and ask the Keycloak admins why the account changed.

1. An admin resets the link: `DELETE /api/people/<user id>/keycloak-link` (admins only, recorded in the activity log). It ends the person's sessions. Without a portal admin, run:

   ```sql
   BEGIN;
   UPDATE users SET keycloak_sub = NULL WHERE andrew_id = '<andrew id>';
   DELETE FROM sessions WHERE user_id = (SELECT id FROM users WHERE andrew_id = '<andrew id>');
   COMMIT;
   ```

1. The person signs in again. The new account is linked only if it proves the Andrew ID (contract, "Keycloak sign-in"), and their roles stay.

## Keycloak was rebuilt and everyone's account changed

Every person who signed in before gets `linked_elsewhere`. Reset every link and end every session; each person is linked again at their next sign-in, with the same proof:

```sql
BEGIN;
UPDATE users SET keycloak_sub = NULL WHERE keycloak_sub IS NOT NULL;
DELETE FROM sessions;
COMMIT;
```

## Someone gets "couldn't match this account to a CMU Andrew ID"

The sign-in page says `error=no_andrew_id`; the activity log has "Refused a ScottyLabs sign-in as ..." with a reason, and the log line `refused a Keycloak sign-in that could not be matched to a CMU Andrew ID` has the same `reason`:

- `email_not_andrew_id`: the account's email is not `<andrew id>@andrew.cmu.edu` or `<andrew id>@cmu.edu`. The person signed in with something other than their Andrew account.
- `email_not_verified`: Keycloak says the email is not verified.
- `no_email`: the token has no email. Check that the client still has the `email` scope.
- `username_not_an_andrew_id`: the username is not shaped like an Andrew ID.
- `no_andrew_id_claim`: `ANDREW_ID_CLAIM` is set and the token lacks that claim. Check the Keycloak mapper.

Each sign-in also logs `Keycloak sign-in: the ID token's email_verified claim` with `true`, `false` or `missing`, without personal data. If real CMU accounts show `missing` or `false`, or their email is not the Andrew ID, ask the Keycloak admins for a mapper that puts the Andrew ID from CMU's directory in its own claim, and set `ANDREW_ID_CLAIM` to that claim's name.

## Keycloak sign-in is off in production

Startup logs `Keycloak sign-in is off: on a production host Keycloak must redirect straight to this site's callback ...` and `/api/auth/config` says `oidc: false`. `OAUTH_RELAY_URL` names the shared Ricochet relay, which also forwards codes to every `*.scottylabs.net` deployment, so the service will not use it on a production host. The fix is in governance: give the prod client `https://scottylabs.org/api/auth/callback` as its only redirect URI and set the prod profile's `OAUTH_RELAY_URL` to the same URL, together (contract, "Ricochet relay").

## A session must end now

For a stolen cookie, someone who left, or anything suspicious:

1. An admin signs the person out everywhere: `DELETE /api/people/<user id>/sessions` (recorded in the activity log). Their roles stay.

1. If they are an admin through the Keycloak admin group, also remove them from the group in governance; otherwise their next sign-in, and any new session's check with Keycloak, gives the role back. Removing their last role on the People page also signs them out everywhere.

1. Without a portal admin:

   ```sql
   DELETE FROM sessions WHERE user_id = (SELECT id FROM users WHERE andrew_id = '<andrew id>');
   ```

Disabling the person's Keycloak account ends their Keycloak-backed sessions within about 15 minutes of their next request, when the session is checked with Keycloak again.

## After restoring the site's database from a backup

The backup brings back sessions that were signed out since, and roles that were removed since. End every session and sign-in attempt, then redo the role changes made after the backup:

```sql
BEGIN;
DELETE FROM sessions;
DELETE FROM auth_attempts;
COMMIT;
```

## While Keycloak is down

- Sign-in says Keycloak is not responding (`error=unavailable`) within about 5 seconds, and does not ask Keycloak again for 30 seconds.

- Existing sessions keep working (12 hours idle, 7 days at most). An admin role that comes only from the Keycloak group stops counting 24 hours after the session was last confirmed with Keycloak, until Keycloak answers again. Admins added on the People page keep admin.

- To take content down without anyone signed in:

  ```sql
  -- End a live announcement.
  UPDATE announcements SET state = 'ended', ended_at = now(), updated_at = now() WHERE id = '<announcement id>';
  -- Take a project off the site.
  UPDATE projects SET state = 'archived', updated_at = now() WHERE slug = '<slug>';
  ```

  Committee pages have no unpublished state. Publish a corrected page from a live admin or director session instead.
