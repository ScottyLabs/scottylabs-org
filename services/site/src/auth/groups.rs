//! The admin role that comes from Keycloak's `PROJECT_ADMIN_GROUP`, applied at sign-in and whenever a session is confirmed with Keycloak again.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::audit;
use crate::error::AppResult;

pub fn same_group(a: &str, b: &str) -> bool {
    a.trim_start_matches('/') == b.trim_start_matches('/')
}

/// Members of `admin_group` are admins. An admin role that came only from the group goes away when the token shows they left it (any `groups` list without the group, including an empty one), unless they are the last admin. Tokens without a `groups` claim (`None`) change nothing.
pub async fn sync_admin_group(
    conn: &mut PgConnection,
    user_id: Uuid,
    name: &str,
    groups: Option<&[String]>,
    admin_group: &str,
) -> AppResult<()> {
    let Some(groups) = groups else {
        return Ok(());
    };
    if groups.iter().any(|group| same_group(group, admin_group)) {
        let granted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, source)
             SELECT $1, $2, 'admin', NULL, true, 'keycloak_group'
             WHERE NOT EXISTS (SELECT 1 FROM role_assignments WHERE user_id = $2 AND role = 'admin')
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await?;
        if granted.is_some() {
            audit::record(
                conn,
                audit::Entry {
                    actor: None,
                    action: "role.add",
                    subject_type: audit::subject::PERSON,
                    subject_id: Some(user_id.to_string()),
                    committee: None,
                    summary: format!("{name} became an admin through the Keycloak admin group"),
                },
            )
            .await?;
        }
        return Ok(());
    }

    let admins: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, user_id, source FROM role_assignments WHERE role = 'admin' ORDER BY id FOR UPDATE",
    )
    .fetch_all(&mut *conn)
    .await?;
    let from_group = admins
        .iter()
        .find(|(_, owner, source)| *owner == user_id && source == "keycloak_group");
    if from_group.is_some() && admins.len() == 1 {
        // Kept so the portal always has an admin; another admin should be added, then this one removed on the People page.
        tracing::warn!(
            user = %user_id,
            "kept the last admin although they left the Keycloak admin group"
        );
    }
    if let Some((assignment_id, _, _)) = from_group
        && admins.len() > 1
    {
        sqlx::query("DELETE FROM role_assignments WHERE id = $1")
            .bind(assignment_id)
            .execute(&mut *conn)
            .await?;
        audit::record(
            conn,
            audit::Entry {
                actor: None,
                action: "role.remove",
                subject_type: audit::subject::PERSON,
                subject_id: Some(user_id.to_string()),
                committee: None,
                summary: format!(
                    "{name} is no longer an admin after leaving the Keycloak admin group"
                ),
            },
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_match_with_or_without_a_leading_slash() {
        assert!(same_group(
            "/projects/scottylabs-org/admins",
            "projects/scottylabs-org/admins"
        ));
        assert!(!same_group(
            "/projects/scottylabs-org",
            "/projects/scottylabs-org/admins"
        ));
    }
}
