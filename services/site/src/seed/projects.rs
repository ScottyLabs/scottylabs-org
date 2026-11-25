//! Seeds the published projects from the approved Projects page designs (Figma "Option C v2") and the home page, with their screenshots and photos.
//!
//! A project is inserted only when no project has, or used to have, its address, so edits, renames and archiving survive restarts. Every repository and site link was checked to exist; links that did not resolve were left out. Terrier has no picture yet, so its card shows the monogram, as in the design.

use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::config::Config;

/// A bundled image with its size from `seed/images.json`.
struct SeedImage {
    src: &'static str,
    width: u32,
    height: u32,
    alt: &'static str,
}

const fn photo(src: &'static str, width: u32, height: u32, alt: &'static str) -> SeedImage {
    SeedImage {
        src,
        width,
        height,
        alt,
    }
}

const TARTANHACKS_TEAM: SeedImage = photo(
    "/seed/photos/c01.jpg",
    1000,
    667,
    "TartanHacks organizers in matching green sweatshirts on the steps of a campus building, under a TartanHacks banner",
);
const PACKED_HALL: SeedImage = photo(
    "/seed/photos/c04.jpg",
    1000,
    667,
    "A packed lecture hall of students watching a talk",
);
const ORGANIZERS_ON_STAGE: SeedImage = photo(
    "/seed/photos/t07.jpg",
    1000,
    667,
    "A row of student organizers on stage in front of a ScottyLabs banner",
);
const TARTANHACKS_MERCH: SeedImage = photo(
    "/seed/photos/c05.jpg",
    1000,
    667,
    "TartanHacks bandanas and shirts printed with the event\u{2019}s anchor and compass artwork",
);
const TARTANHACKS_CLOSING: SeedImage = photo(
    "/seed/photos/t09.jpg",
    1000,
    680,
    "The TartanHacks team in green sweatshirts in front of a projected TartanHacks title slide",
);
const TARTANHACKS_SITE: SeedImage = photo(
    "/seed/projects/tartanhacks.jpg",
    1440,
    900,
    "The TartanHacks website announcing February 5\u{2013}6, 2027 at Carnegie Mellon, with buttons to pre-register and to sign up to judge",
);

/// One seeded project, published as it is.
struct SeedProject {
    slug: &'static str,
    name: &'static str,
    summary: &'static str,
    /// Paragraphs of the description, as plain text.
    paragraphs: &'static [&'static str],
    committee: &'static str,
    built_with: &'static [&'static str],
    status: &'static str,
    kind: &'static str,
    site_url: Option<&'static str>,
    repo_url: Option<&'static str>,
    media: &'static [SeedImage],
    featured: bool,
}

const PROJECTS: &[SeedProject] = &[
    SeedProject {
        slug: "tartanhacks",
        name: "TartanHacks",
        summary: "Pittsburgh\u{2019}s largest annual hackathon.",
        paragraphs: &[
            "Every February, 1000+ students take over campus for a weekend of building, workshops, boba and midnight snacks. TartanHacks is planned and run entirely by ScottyLabs students.",
            "The 2027 edition runs February 5\u{2013}6 at Carnegie Mellon.",
        ],
        committee: "events",
        built_with: &["tech", "design", "finance"],
        status: "annual",
        kind: "event",
        site_url: Some("https://tartanhacks.com/"),
        repo_url: None,
        media: &[
            TARTANHACKS_TEAM,
            PACKED_HALL,
            ORGANIZERS_ON_STAGE,
            TARTANHACKS_MERCH,
            TARTANHACKS_CLOSING,
            TARTANHACKS_SITE,
        ],
        featured: true,
    },
    SeedProject {
        slug: "cmu-courses",
        name: "CMU Courses",
        summary: "Course search and scheduling.",
        paragraphs: &[
            "Course search and scheduling for Carnegie Mellon students. It brings course information, schedules and FCE data together so you can plan a semester in one place.",
        ],
        committee: "tech",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://www.cmucourses.com/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/courses"),
        media: &[photo(
            "/seed/projects/courses.jpg",
            1440,
            826,
            "CMU Courses showing a course page with its description, schedule and FCE ratings",
        )],
        featured: false,
    },
    SeedProject {
        slug: "nova",
        name: "Nova",
        summary: "ScottyLabs\u{2019} fall hackathon.",
        paragraphs: &[
            "ScottyLabs\u{2019} fall hackathon. Teams build over a weekend, demo to the room, and the crowd votes for its favorite.",
        ],
        committee: "events",
        built_with: &["tech", "design"],
        status: "annual",
        kind: "event",
        site_url: None,
        repo_url: Some("https://git.cmu.dev/ScottyLabs/whistle"),
        media: &[
            photo(
                "/seed/photos/c09.jpg",
                1000,
                667,
                "Four Nova winners standing in front of a screen announcing the Everyday Impact award",
            ),
            photo(
                "/seed/photos/c11.jpg",
                1000,
                667,
                "Hackers at their laptops under a Nova countdown that reads hacking ends in one second",
            ),
        ],
        featured: false,
    },
    SeedProject {
        slug: "cmu-maps",
        name: "CMU Maps",
        summary: "Room-level navigation of campus.",
        paragraphs: &[
            "Room-level, real-time navigation of Carnegie Mellon\u{2019}s campus, indoors and out.",
        ],
        committee: "tech",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://cmumaps.com/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/maps"),
        media: &[photo(
            "/seed/projects/maps.jpg",
            1440,
            900,
            "CMU Maps showing the Carnegie Mellon campus with its buildings colored by type",
        )],
        featured: false,
    },
    SeedProject {
        slug: "cmu-housing",
        name: "CMU Housing",
        summary: "Find the right dorm for you.",
        paragraphs: &[
            "Helps students discover housing options at CMU. Compare dorms, filter by what matters to you, and get personalized results. Built with the Undergraduate Student Senate.",
        ],
        committee: "labrador",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://cmuhousing.com/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/housing"),
        media: &[photo(
            "/seed/projects/housing.jpg",
            1440,
            900,
            "The CMU Housing home page with filters and photos of campus residence halls",
        )],
        featured: false,
    },
    SeedProject {
        slug: "cmu-eats",
        name: "CMU Eats",
        summary: "What\u{2019}s open on campus right now.",
        paragraphs: &[
            "Your one-stop shop for finding out which campus dining locations are open right now.",
        ],
        committee: "tech",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://cmueats.com/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/eats"),
        media: &[photo(
            "/seed/projects/eats.jpg",
            1440,
            900,
            "CMU Eats listing campus dining locations with their opening hours",
        )],
        featured: false,
    },
    SeedProject {
        slug: "tartanhacks-identity",
        name: "TartanHacks identity",
        summary: "Brand, shirts and badges.",
        paragraphs: &["The visual identity for TartanHacks: wordmark, shirts, badges and signage."],
        committee: "design",
        built_with: &["events"],
        status: "annual",
        kind: "brand",
        site_url: None,
        repo_url: None,
        media: &[TARTANHACKS_MERCH, TARTANHACKS_CLOSING, TARTANHACKS_SITE],
        featured: false,
    },
    SeedProject {
        slug: "terrier",
        name: "Terrier",
        summary: "Open-source hackathon platform.",
        paragraphs: &[
            "An open-source hackathon management platform for universities and organizations, written in Rust.",
        ],
        committee: "tech",
        built_with: &[],
        status: "in_development",
        kind: "tool",
        site_url: None,
        repo_url: Some("https://git.cmu.dev/ScottyLabs/terrier"),
        media: &[],
        featured: false,
    },
    SeedProject {
        slug: "speaker-series",
        name: "Speaker series",
        summary: "Founders and investors on campus.",
        paragraphs: &[
            "Founders and investors come to campus to talk with students about building companies.",
        ],
        committee: "foundry",
        built_with: &["events"],
        status: "ongoing",
        kind: "program",
        site_url: None,
        repo_url: None,
        media: &[
            photo(
                "/seed/photos/c14.jpg",
                1000,
                992,
                "A speaker with a microphone beside a Carnegie Mellon University lectern",
            ),
            PACKED_HALL,
        ],
        featured: false,
    },
    SeedProject {
        slug: "bus-sign",
        name: "Bus Sign",
        summary: "Live bus arrivals at the UC.",
        paragraphs: &[
            "A real-time bus sign for the Cohon University Center, showing live PRT arrivals in both directions.",
        ],
        committee: "tech",
        built_with: &[],
        status: "live",
        kind: "tool",
        site_url: Some("https://bus-sign.scottylabs.org/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/bus-sign"),
        media: &[photo(
            "/seed/projects/bussign.jpg",
            1440,
            900,
            "The bus sign showing live inbound and outbound bus arrival times",
        )],
        featured: false,
    },
    SeedProject {
        slug: "giving-day",
        name: "Giving Day",
        summary: "Year-round support for builders.",
        paragraphs: &[
            "ScottyLabs\u{2019} annual fundraising push, supporting student builders year-round.",
        ],
        committee: "finance",
        built_with: &["outreach", "design"],
        status: "annual",
        kind: "fundraiser",
        site_url: None,
        repo_url: None,
        media: &[
            photo(
                "/seed/photos/c15.jpg",
                1000,
                667,
                "Five ScottyLabs members standing behind a table with a ScottyLabs tablecloth",
            ),
            photo(
                "/seed/photos/c16.jpg",
                1000,
                750,
                "A large group of ScottyLabs members outside on a campus lawn",
            ),
        ],
        featured: false,
    },
    SeedProject {
        slug: "cmu-study",
        name: "CMU Study",
        summary: "Create and join study groups.",
        paragraphs: &["A platform for creating and joining study groups for your CMU courses."],
        committee: "labrador",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://cmustudy.com/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/study"),
        media: &[photo(
            "/seed/projects/study.jpg",
            1440,
            900,
            "The CMU Study home page, headed Find your people. Get the work done., above a list of study groups",
        )],
        featured: false,
    },
    SeedProject {
        slug: "orientation-quest",
        name: "Orientation Quest",
        summary: "First-year scavenger hunt.",
        paragraphs: &[
            "A scavenger hunt for first-year orientation. New students explore campus, complete challenges, and earn ScottyCoins and points for the Carnegie Cup.",
        ],
        committee: "outreach",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://cmu.quest/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/quest"),
        media: &[photo(
            "/seed/projects/quest.jpg",
            1440,
            900,
            "The CMU Orientation Quest page with links to download the app from the App Store and Google Play",
        )],
        featured: false,
    },
    SeedProject {
        slug: "tartan-vote",
        name: "Tartan Vote",
        summary: "Elections for Senate and student orgs.",
        paragraphs: &["A voting app for the Undergraduate Senate and other student organizations."],
        committee: "tech",
        built_with: &[],
        status: "live",
        kind: "app",
        site_url: Some("https://tartan.vote/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/tartan-vote"),
        media: &[photo(
            "/seed/projects/vote.jpg",
            1440,
            900,
            "The Tartan Vote sign-in page, which asks voters to sign in with CMU single sign-on",
        )],
        featured: false,
    },
    SeedProject {
        slug: "scottylabs-docs",
        name: "ScottyLabs Docs",
        summary: "One home for project documentation.",
        paragraphs: &["A centralized documentation hub for ScottyLabs projects."],
        committee: "tech",
        built_with: &[],
        status: "live",
        kind: "tool",
        site_url: Some("https://docs.scottylabs.org/"),
        repo_url: Some("https://git.cmu.dev/ScottyLabs/documentation"),
        media: &[photo(
            "/seed/projects/docs.jpg",
            1440,
            900,
            "The ScottyLabs Docs site explaining how each project\u{2019}s documentation is collected",
        )],
        featured: false,
    },
];

/// The lead persona who edits Nova in demo content.
const NOVA_EDITOR: &str = "spchoksh";

pub async fn run(conn: &mut PgConnection, config: &Config) -> anyhow::Result<()> {
    for project in PROJECTS {
        let inserted = insert_if_new(conn, project).await?;
        if let Some(id) = inserted.filter(|_| project.slug == "nova" && config.seed_demo) {
            add_editor(conn, id, NOVA_EDITOR).await?;
        }
    }
    Ok(())
}

/// The `ProjectDoc` for a seeded project.
fn doc(project: &SeedProject) -> Value {
    let description: String = project
        .paragraphs
        .iter()
        .map(|text| format!("<p>{}</p>", escape(text)))
        .collect();
    let media: Vec<Value> = project
        .media
        .iter()
        .map(|image| {
            json!({ "src": image.src, "alt": image.alt, "width": image.width, "height": image.height })
        })
        .collect();
    json!({
        "name": project.name,
        "summary": project.summary,
        "description": description,
        "committee": project.committee,
        "builtWith": project.built_with,
        "status": project.status,
        "kind": project.kind,
        "siteUrl": project.site_url,
        "repoUrl": project.repo_url,
        "videoUrl": null,
        "media": media,
    })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Inserts a published project unless its address is taken or was used before. Returns its id when inserted. It is featured only while no other project is, since one is featured at a time.
async fn insert_if_new(
    conn: &mut PgConnection,
    project: &SeedProject,
) -> anyhow::Result<Option<Uuid>> {
    let doc = doc(project);
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO projects (id, slug, committee, state, draft, published, featured, published_at)
         SELECT $1, $2, $3, 'published', $4, $4,
                $5 AND NOT EXISTS (SELECT 1 FROM projects WHERE featured), now()
         WHERE NOT EXISTS (SELECT 1 FROM project_slug_redirects WHERE old_slug = $2)
         ON CONFLICT (slug) DO NOTHING
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(project.slug)
    .bind(project.committee)
    .bind(&doc)
    .bind(project.featured)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(id)
}

/// Lists a persona as an editor, when they exist (demo content only).
async fn add_editor(conn: &mut PgConnection, project: Uuid, andrew_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO project_editors (project_id, user_id)
         SELECT $1, id FROM users WHERE andrew_id = $2
         ON CONFLICT (project_id, user_id) DO NOTHING",
    )
    .bind(project)
    .bind(andrew_id)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_addresses_are_unique_and_one_project_is_featured() {
        let mut slugs: Vec<&str> = PROJECTS.iter().map(|p| p.slug).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), PROJECTS.len());
        assert_eq!(PROJECTS.iter().filter(|p| p.featured).count(), 1);
    }

    #[test]
    fn descriptions_are_escaped_paragraphs() {
        assert_eq!(escape("a < b & c"), "a &lt; b &amp; c");
        let tartanhacks = doc(&PROJECTS[0]);
        let description = tartanhacks["description"].as_str().unwrap();
        assert_eq!(description.matches("<p>").count(), 2);
    }
}
