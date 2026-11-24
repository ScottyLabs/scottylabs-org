//! Demo people for dev sign-in, seeded only when `SEED_DEMO_CONTENT=1`. A persona's role is added only when the persona itself is first created, so removing a role in the portal sticks across restarts. The projects seed makes Sara an editor of Nova.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::config::Config;
use crate::models::{Committee, Role};

/// Someone dev sign-in can act as.
pub struct Persona {
    pub andrew_id: &'static str,
    pub name: &'static str,
    /// How the sign-in page describes them, for example "Director of Events".
    pub label: &'static str,
    pub role: Option<PersonaRole>,
}

pub struct PersonaRole {
    pub role: Role,
    pub committee: Option<Committee>,
    pub can_post_ticker: bool,
}

pub const PERSONAS: [Persona; 5] = [
    Persona {
        andrew_id: "tkanz",
        name: "Thomas Kanz",
        label: "Admin",
        role: Some(PersonaRole {
            role: Role::Admin,
            committee: None,
            can_post_ticker: true,
        }),
    },
    Persona {
        andrew_id: "evanfost",
        name: "Evan Foster",
        label: "Director of Events",
        role: Some(PersonaRole {
            role: Role::Director,
            committee: Some(Committee::Events),
            can_post_ticker: true,
        }),
    },
    Persona {
        andrew_id: "anishp",
        name: "Anish Pallati",
        label: "Director of Tech",
        role: Some(PersonaRole {
            role: Role::Director,
            committee: Some(Committee::Tech),
            can_post_ticker: true,
        }),
    },
    Persona {
        andrew_id: "spchoksh",
        name: "Sara Chokshi",
        label: "Lead in Events",
        role: Some(PersonaRole {
            role: Role::Lead,
            committee: Some(Committee::Events),
            can_post_ticker: true,
        }),
    },
    Persona {
        andrew_id: "newmember",
        name: "New Member",
        label: "No role yet",
        role: None,
    },
];

pub fn persona(andrew_id: &str) -> Option<&'static Persona> {
    PERSONAS.iter().find(|p| p.andrew_id == andrew_id)
}

pub async fn run(conn: &mut PgConnection, config: &Config) -> anyhow::Result<()> {
    if !config.seed_demo {
        return Ok(());
    }
    for persona in &PERSONAS {
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO users (id, andrew_id, name) VALUES ($1, $2, $3)
             ON CONFLICT (andrew_id) DO NOTHING
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(persona.andrew_id)
        .bind(persona.name)
        .fetch_optional(&mut *conn)
        .await?;
        let (Some(user_id), Some(role)) = (inserted, &persona.role) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, source)
             VALUES ($1, $2, $3, $4, $5, 'portal')",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(role.role)
        .bind(role.committee)
        .bind(role.can_post_ticker)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::testing;

    #[sqlx::test(migrations = "./migrations")]
    async fn seeds_personas_once_and_keeps_edits(db: PgPool) {
        testing::seed_personas(&db).await;
        let roles: i64 = sqlx::query_scalar("SELECT count(*) FROM role_assignments")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(roles, 4);

        // Someone removes Sara's role; seeding again does not bring it back.
        sqlx::query(
            "DELETE FROM role_assignments WHERE user_id = (SELECT id FROM users WHERE andrew_id = 'spchoksh')",
        )
        .execute(&db)
        .await
        .unwrap();
        testing::seed_personas(&db).await;
        let roles: i64 = sqlx::query_scalar("SELECT count(*) FROM role_assignments")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(roles, 3);
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(users, 5);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn skips_personas_without_demo_content(db: PgPool) {
        testing::ensure_committees(&db).await;
        let mut config = testing::config();
        config.seed_demo = false;
        let mut conn = db.acquire().await.unwrap();
        run(&mut conn, &config).await.unwrap();
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(users, 0);
    }
}
