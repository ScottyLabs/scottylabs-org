//! Seeds the seven committee pages with the copy, block order and widths from the approved designs, published as they are. Pages that already exist are left alone, so edits survive restarts.
//!
//! The designs show two video blocks (the TartanHacks recap and a Foundry fireside chat) without real videos, so they are image blocks with the same photo and caption until a committee adds a link.

use std::collections::HashMap;

use anyhow::Context;
use serde::Deserialize;
use sqlx::PgConnection;

use crate::blocks::{
    Block, BlockContent, CommitteePageDoc, CommitteeSettings, CoverProps, CtaProps, GalleryLayout,
    GalleryProps, ImageProps, LinkButton, ProjectsProps, QuoteProps, Stat, StatsProps, Team,
    TeamsProps, TextProps, WorkSessionsProps,
};
use crate::config::Config;
use crate::models::{Committee, ImageRef};

/// The club sign-up link the home page uses.
const JOIN_URL: &str = "https://tartanconnect.cmu.edu/scottylabs/club_signup";
/// The Slack invite the site footer uses.
const SLACK_URL: &str = "https://go.scottylabs.org/slack";
const MEETING_TIME: &str = "Saturdays, 4\u{2013}6 PM";
const SESSION_TEXT: &str =
    "Come to a Saturday work session, or apply when recruitment opens each semester.";

pub async fn run(conn: &mut PgConnection, _config: &Config) -> anyhow::Result<()> {
    let images = SeedImages::load()?;
    for committee in Committee::ALL {
        let doc = page(*committee, &images);
        sqlx::query(
            "INSERT INTO committee_pages (committee, draft, published, draft_version, published_at)
             VALUES ($1, $2, $2, 1, now())
             ON CONFLICT (committee) DO NOTHING",
        )
        .bind(committee)
        .bind(sqlx::types::Json(&doc))
        .execute(&mut *conn)
        .await
        .with_context(|| format!("seeding the {} page", committee.name()))?;
    }
    Ok(())
}

/// Sizes of the photos bundled in sites/web/static/seed/, so pages reserve the right space.
struct SeedImages(HashMap<String, Size>);

#[derive(Deserialize)]
struct Size {
    width: u32,
    height: u32,
}

impl SeedImages {
    fn load() -> anyhow::Result<Self> {
        serde_json::from_str(include_str!("../../seed/images.json"))
            .map(Self)
            .context("reading seed/images.json")
    }

    fn get(&self, src: &str, alt: &str) -> ImageRef {
        let size = self.0.get(src);
        ImageRef {
            src: src.into(),
            alt: alt.into(),
            width: size.map(|s| s.width),
            height: size.map(|s| s.height),
        }
    }
}

/// The seeded page for a committee.
fn page(committee: Committee, images: &SeedImages) -> CommitteePageDoc {
    let slug = committee.slug();
    let settings =
        |index_description: &str, teams_line: &str, room: &str, map: &str| CommitteeSettings {
            index_description: index_description.into(),
            teams_line: teams_line.into(),
            meeting_time: MEETING_TIME.into(),
            meeting_room: room.into(),
            map_url: Some(format!("https://maps.scottylabs.org/{map}")),
            slack_channel: Some(slug.into()),
        };
    let join = |heading: &str| {
        cta(
            heading,
            SESSION_TEXT,
            (&format!("Apply to {}", committee.name()), JOIN_URL),
            (&format!("Message #{slug}"), SLACK_URL),
        )
    };
    let img = |src: &str, alt: &str| images.get(src, alt);

    match committee {
        Committee::Events => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We run the weekends CMU builders remember.",
                    img(
                        "/seed/photos/c04.jpg",
                        "A packed lecture hall of students at TartanHacks.",
                    ),
                ),
                text(
                    "what-we-do",
                    6,
                    "What we do",
                    &[
                        "Events plans and runs the weekends people remember: TartanHacks in February, NOVA, and the smaller build nights through the year. We book the rooms, feed the hackers, line up judges and keep the schedule honest.",
                        "Members own a real piece of a real event from their first semester, with a team around them that has run it before.",
                    ],
                ),
                image(
                    "recap",
                    6,
                    img(
                        "/seed/photos/c01.jpg",
                        "The TartanHacks volunteer team in matching sweaters on the steps outside the venue.",
                    ),
                    "TartanHacks recap",
                ),
                stats(&[
                    ("1,000+", "participants at TartanHacks"),
                    ("2", "hackathons a year: TartanHacks and NOVA"),
                    ("February", "when TartanHacks takes over campus"),
                ]),
                gallery(
                    GalleryLayout::Mosaic,
                    vec![
                        img(
                            "/seed/photos/t09.jpg",
                            "The TartanHacks organizing team on stage at the closing ceremony.",
                        ),
                        img(
                            "/seed/photos/c03.jpg",
                            "Volunteers in TartanHacks shirts talking in a hallway.",
                        ),
                        img(
                            "/seed/photos/t03.jpg",
                            "A volunteer waiting at the TartanHacks check-in table.",
                        ),
                    ],
                    "TartanHacks: closing ceremony, the volunteer team and check-in.",
                ),
                teams(
                    12,
                    "Three teams run the calendar. Talk to the one you want to help.",
                    &[
                        (
                            "Logistics",
                            "Venues, food, schedules and everything that has to work on the day.",
                            "events-logistics",
                        ),
                        (
                            "TartanHacks",
                            "Pittsburgh\u{2019}s largest annual hackathon, every February.",
                            "tartanhacks",
                        ),
                        (
                            "NOVA",
                            "Our AI hackathon, built around real problems.",
                            "nova",
                        ),
                    ],
                ),
                join("Help run TartanHacks"),
            ],
            settings: settings(
                "Plans and runs TartanHacks, Pittsburgh\u{2019}s largest annual hackathon, plus NOVA and the build nights in between. If it has a schedule, a venue and a thousand hungry hackers, Events owns it.",
                "3 teams: Logistics, TartanHacks and NOVA",
                "Tepper 3808",
                "TEP-3808",
            ),
        },
        Committee::Outreach => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We make sure campus knows what ScottyLabs is building.",
                    img(
                        "/seed/photos/c02.jpg",
                        "Students visiting project tables at a ScottyLabs event.",
                    ),
                ),
                text(
                    "what-we-do",
                    12,
                    "What we do",
                    &[
                        "Outreach is how campus hears about ScottyLabs. We run marketing and social, build partnerships with companies and other student groups, and host the dinners and socials that keep members around.",
                        "We also work with project teams on launches, so the things Tech and Labrador build reach the students they were made for.",
                    ],
                ),
                gallery(
                    GalleryLayout::Thirds,
                    vec![
                        img(
                            "/seed/photos/c15.jpg",
                            "ScottyLabs members behind a table draped with the ScottyLabs banner.",
                        ),
                        img(
                            "/seed/photos/c16.jpg",
                            "A large group of ScottyLabs members on a lawn on campus.",
                        ),
                        img(
                            "/seed/photos/t12.jpg",
                            "Students talking in a crowded room during a ScottyLabs event.",
                        ),
                    ],
                    "",
                ),
                teams(
                    12,
                    "Seven teams, each with its own channel.",
                    &[
                        (
                            "Marketing",
                            "Campaigns, social and the ScottyLabs voice.",
                            "outreach-marketing",
                        ),
                        (
                            "External Partnerships",
                            "Companies and groups we work with outside CMU.",
                            "outreach-partners",
                        ),
                        (
                            "Campus Partnerships",
                            "Other clubs, departments and student government.",
                            "outreach-campus",
                        ),
                        (
                            "Community Engagement",
                            "Socials, dinners and the member community.",
                            "outreach-community",
                        ),
                        (
                            "Product Strategy",
                            "Helps project teams decide what to build next.",
                            "outreach-product",
                        ),
                        (
                            "Go-To-Market",
                            "Launch plans for new apps and features.",
                            "outreach-gtm",
                        ),
                        (
                            "Growth Analytics",
                            "Measures who uses our apps and why.",
                            "outreach-growth",
                        ),
                    ],
                ),
                work_sessions(4),
                quote(
                    8,
                    "The best part is watching someone use an app we helped launch without knowing we exist.",
                    "An Outreach member",
                ),
                join("Tell the ScottyLabs story"),
            ],
            settings: settings(
                "Tells the ScottyLabs story on campus and beyond. Outreach runs marketing, partnerships and community events, and helps every project find the students who need it.",
                "7 teams, including Marketing, Partnerships and Community",
                "Tepper 3808",
                "TEP-3808",
            ),
        },
        Committee::Tech => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We build the apps CMU opens every day.",
                    img(
                        "/seed/photos/c10.jpg",
                        "Tech members smiling over their laptops at a hackathon.",
                    ),
                ),
                text(
                    "what-we-do",
                    8,
                    "What we do",
                    &[
                        "Tech is where ScottyLabs builds. We design, ship and maintain the apps students use to find a classroom, pick a course or see what is open for dinner, and we keep them running year after year.",
                        "Every project has its own team and tech lead. New members join a team in their first semester and ship real code, with review from people who have done it before. Shared infrastructure, a design system and a DevOps team mean your time goes into the product.",
                    ],
                ),
                work_sessions(4),
                stats(&[
                    ("15", "project teams"),
                    ("~80%", "of campus uses something we built"),
                    ("2011", "building for CMU since"),
                ]),
                projects("Projects from Tech"),
                teams(
                    12,
                    "Fifteen teams, each with its own channel. Talk to the one you are curious about.",
                    &[
                        (
                            "CMU Maps",
                            "Room-level navigation for every building on campus.",
                            "tech-maps",
                        ),
                        (
                            "CMU Courses",
                            "Course search, reviews and schedule planning.",
                            "tech-courses",
                        ),
                        (
                            "CMU Eats",
                            "What is open to eat on campus right now.",
                            "tech-eats",
                        ),
                        (
                            "CMU Housing",
                            "Compare dorms and off-campus options.",
                            "tech-housing",
                        ),
                        (
                            "CMU Study",
                            "Find a free study room or a study group.",
                            "tech-study",
                        ),
                        (
                            "CMU Cal",
                            "One calendar for campus events and deadlines.",
                            "tech-cal",
                        ),
                        (
                            "Lost and Found",
                            "Report and recover lost items on campus.",
                            "tech-lost-found",
                        ),
                        (
                            "Tartan.vote",
                            "Student government elections, online.",
                            "tech-vote",
                        ),
                        (
                            "O-Quest",
                            "The NFC scavenger hunt for Orientation Week.",
                            "tech-oquest",
                        ),
                        (
                            "Terrier",
                            "The platform that runs TartanHacks.",
                            "tech-terrier",
                        ),
                        (
                            "ScottyLabs AI",
                            "Applied AI projects for campus.",
                            "tech-ai",
                        ),
                        (
                            "Open Compute Cluster",
                            "Shared compute for student projects and clubs.",
                            "tech-compute",
                        ),
                        (
                            "DevOps and Internal Tooling",
                            "Deploys, secrets and the tools every team shares.",
                            "tech-devops",
                        ),
                        (
                            "UI Architecture",
                            "The component library behind our apps.",
                            "tech-ui",
                        ),
                        (
                            "Community-Based Projects",
                            "Software built with Pittsburgh community partners.",
                            "tech-community",
                        ),
                    ],
                ),
                quote(
                    6,
                    "I joined to fix one bug in CMU Maps. Two semesters later I was leading the team.",
                    "A CMU Maps team member",
                ),
                image(
                    "work-session",
                    6,
                    img(
                        "/seed/photos/t02.jpg",
                        "Students working on laptops at a Saturday work session.",
                    ),
                    "A Saturday work session in Tepper.",
                ),
                join("Build with Tech"),
            ],
            settings: settings(
                "Builds and maintains the apps CMU students open every day, from CMU Maps and CMU Courses to CMU Eats. Fifteen project teams ship in the open, with shared infrastructure and design support.",
                "15 teams, including CMU Maps, CMU Courses and CMU Eats",
                "Tepper 2700, 2701, 2613",
                "TEP-2700",
            ),
        },
        Committee::Labrador => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We turn student ideas into first products.",
                    img(
                        "/seed/photos/c13.jpg",
                        "A small robot arm a student team built, on a table beside a laptop.",
                    ),
                ),
                text(
                    "how-it-works",
                    8,
                    "How Labrador works",
                    &[
                        "Labrador is where a student idea becomes a first product. You bring your own idea. We give you structured Saturday work sessions, a mentor who has shipped before, and a demo date to work toward.",
                        "Teams from Labrador have launched products like CollegeCart, with design and engineering support from across ScottyLabs.",
                    ],
                ),
                work_sessions(4),
                projects("Built in Labrador"),
                teams(
                    12,
                    "Six teams support every founder in the program.",
                    &[
                        (
                            "Product",
                            "Helps founders find a problem worth solving.",
                            "labrador-product",
                        ),
                        (
                            "Innovation",
                            "Runs idea nights and scouts new directions.",
                            "labrador-innovation",
                        ),
                        (
                            "Mentorship",
                            "Pairs every team with someone who has shipped before.",
                            "labrador-mentors",
                        ),
                        (
                            "Engineering",
                            "Hands-on build help for early prototypes.",
                            "labrador-eng",
                        ),
                        (
                            "AI",
                            "Support for teams building with models.",
                            "labrador-ai",
                        ),
                        (
                            "Onboarding and Documentation",
                            "Gets new members building in their first week.",
                            "labrador-onboarding",
                        ),
                    ],
                ),
                image(
                    "demo-night",
                    6,
                    img(
                        "/seed/photos/c09.jpg",
                        "Four students in front of the NOVA screen at demo night.",
                    ),
                    "Demo night winners.",
                ),
                quote(
                    6,
                    "Having a demo date changed everything. We stopped planning and started building.",
                    "A Labrador founder",
                ),
                join("Bring your idea to Labrador"),
            ],
            settings: settings(
                "The zero-to-one incubator. Members bring their own ideas, then get structured Saturday work sessions, mentorship and a demo date to turn them into products like CollegeCart.",
                "6 teams, including Product, Mentorship and Engineering",
                "Swartz Center (Tepper 3713)",
                "TEP-3713",
            ),
        },
        Committee::Design => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We decide how ScottyLabs looks, reads and feels.",
                    img(
                        "/seed/photos/c05.jpg",
                        "TartanHacks shirts and tote bags printed with the event\u{2019}s anchor artwork.",
                    ),
                ),
                image(
                    "designer",
                    4,
                    img(
                        "/seed/photos/t01.jpg",
                        "A designer in a TartanHacks sweater working on a laptop at a busy table.",
                    ),
                    "",
                ),
                text(
                    "what-we-do",
                    8,
                    "What we do",
                    &[
                        "Design decides how ScottyLabs looks, reads and feels. UI/UX designers work inside project teams from the first sketch to launch, and the Branding team builds the identity for TartanHacks, NOVA and the club itself.",
                        "You will ship work that thousands of students see, with critique from designers a year or two ahead of you.",
                    ],
                ),
                gallery(
                    GalleryLayout::Thirds,
                    vec![
                        img(
                            "/seed/projects/tartanhacks.jpg",
                            "The TartanHacks website with its registration countdown.",
                        ),
                        img(
                            "/seed/projects/components.jpg",
                            "The ScottyLabs Components documentation site.",
                        ),
                        img(
                            "/seed/photos/c11.jpg",
                            "Hackers in front of the NOVA countdown on a large screen.",
                        ),
                    ],
                    "TartanHacks identity, the ScottyLabs component library and NOVA on the big screen.",
                ),
                teams(
                    8,
                    "Two teams. Most designers work with both.",
                    &[
                        (
                            "UI/UX",
                            "Designers working inside project teams on product design.",
                            "design-uiux",
                        ),
                        (
                            "Branding",
                            "Identity for TartanHacks, NOVA and ScottyLabs itself.",
                            "design-brand",
                        ),
                    ],
                ),
                work_sessions(4),
                quote(
                    12,
                    "My first project shipped to the whole campus before my first studio critique.",
                    "A UI/UX designer",
                ),
                join("Design with us"),
            ],
            settings: settings(
                "Shapes how every ScottyLabs product and event looks and feels. Designers work inside project teams on UI and UX, and run the brand for TartanHacks and the club.",
                "2 teams: UI/UX and Branding",
                "Tepper 3801",
                "TEP-3801",
            ),
        },
        Committee::Finance => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We keep the lights on and the events free.",
                    img(
                        "/seed/photos/t04.jpg",
                        "Students in TartanHacks shirts printed with the sponsors\u{2019} names.",
                    ),
                ),
                text(
                    "what-we-do",
                    6,
                    "What we do",
                    &[
                        "Finance keeps ScottyLabs funded and accountable. We manage the budget, work with sponsors and university offices, and make sure every committee has what it needs.",
                        "Sponsorship is why TartanHacks stays free for hackers and why our apps have the infrastructure to stay online.",
                    ],
                ),
                image(
                    "sponsor-booths",
                    6,
                    img(
                        "/seed/photos/c12.jpg",
                        "Students talking with sponsors at their booths.",
                    ),
                    "Sponsor booths at TartanHacks.",
                ),
                teams(
                    8,
                    "Two teams. Reach out to either one directly.",
                    &[
                        (
                            "Sponsorship",
                            "Works with the companies that fund our events and apps.",
                            "finance-sponsors",
                        ),
                        (
                            "University Relations",
                            "Budgets, funding requests and CMU offices.",
                            "finance-university",
                        ),
                    ],
                ),
                work_sessions(4),
                cta(
                    "Sponsor ScottyLabs",
                    "Sponsors keep TartanHacks free and our apps online. See the options or talk to the Finance team.",
                    ("See sponsorship options", "/sponsors/"),
                    ("Email Finance", "mailto:hello@scottylabs.org"),
                ),
            ],
            settings: settings(
                "Keeps ScottyLabs funded and accountable. Finance manages the budget and works with sponsors so events stay free and projects have the infrastructure they need.",
                "2 teams: Sponsorship and University Relations",
                "Tepper 3808",
                "TEP-3808",
            ),
        },
        Committee::Foundry => CommitteePageDoc {
            blocks: vec![
                cover(
                    "We back CMU founders building real companies.",
                    img(
                        "/seed/photos/c07.jpg",
                        "Founders working at laptops in a startup space on campus.",
                    ),
                ),
                text(
                    "what-foundry-is",
                    8,
                    "What Foundry is",
                    &[
                        "Foundry is for founders who are past the idea stage and working seriously on a company. Members join a cohort, get time with investors and operators, and work alongside other founders at the same stage.",
                        "Analysts and the Talent and VC Relations teams support the cohort with research, introductions and events.",
                    ],
                ),
                work_sessions(4),
                stats(&[
                    ("$12M+", "raised by Foundry founders"),
                    ("50+", "venture firms in the network"),
                    ("30+", "fireside chats in one semester"),
                ]),
                image(
                    "fireside-chat",
                    6,
                    img(
                        "/seed/photos/c14.jpg",
                        "A speaker at a Carnegie Mellon podium during a fireside chat.",
                    ),
                    "Fireside chat: raising a first round",
                ),
                quote(
                    6,
                    "The fireside chats were useful. The other founders in the room were the reason I stayed.",
                    "A Foundry cohort member",
                ),
                teams(
                    12,
                    "Three teams keep the cohort moving.",
                    &[
                        (
                            "Talent",
                            "Connects founders with teammates and early hires.",
                            "foundry-talent",
                        ),
                        (
                            "Analysts",
                            "Research companies and support the cohort.",
                            "foundry-analysts",
                        ),
                        (
                            "VC Relations",
                            "Keeps our investor network warm and useful.",
                            "foundry-vc",
                        ),
                    ],
                ),
                gallery(
                    GalleryLayout::Thirds,
                    vec![
                        img(
                            "/seed/photos/c16.jpg",
                            "A large group of ScottyLabs members on a lawn on campus.",
                        ),
                        img(
                            "/seed/photos/t11.jpg",
                            "A room of students working at tables during an event.",
                        ),
                        img(
                            "/seed/photos/c06.jpg",
                            "Graduating ScottyLabs members in caps, gowns and stoles on a lawn.",
                        ),
                    ],
                    "",
                ),
                cta(
                    "Apply to the next cohort",
                    "Cohort applications open each semester. Fireside chats are open to every ScottyLabs member.",
                    ("Apply to Foundry", JOIN_URL),
                    ("Message #foundry", SLACK_URL),
                ),
            ],
            settings: settings(
                "For founders past the idea stage. Foundry backs students working seriously on their companies with a cohort, fireside chats with investors and a network of venture firms.",
                "3 teams: Talent, Analysts and VC Relations",
                "Tepper 2612",
                "TEP-2612",
            ),
        },
    }
}

fn block(id: &str, width: u8, content: BlockContent) -> Block {
    Block {
        id: id.into(),
        width,
        content,
    }
}

fn cover(tagline: &str, image: ImageRef) -> Block {
    block(
        "cover",
        12,
        BlockContent::Cover(CoverProps {
            tagline: tagline.into(),
            image: Some(image),
        }),
    )
}

/// Paragraphs of plain text as rich text. The copy has no markup characters to escape.
fn text(id: &str, width: u8, heading: &str, paragraphs: &[&str]) -> Block {
    let body = paragraphs.iter().map(|p| format!("<p>{p}</p>")).collect();
    block(
        id,
        width,
        BlockContent::Text(TextProps {
            heading: heading.into(),
            body,
        }),
    )
}

fn image(id: &str, width: u8, image: ImageRef, caption: &str) -> Block {
    block(
        id,
        width,
        BlockContent::Image(ImageProps {
            image: Some(image),
            caption: caption.into(),
        }),
    )
}

fn gallery(layout: GalleryLayout, images: Vec<ImageRef>, caption: &str) -> Block {
    block(
        "gallery",
        12,
        BlockContent::Gallery(GalleryProps {
            images,
            layout,
            caption: caption.into(),
        }),
    )
}

fn stats(items: &[(&str, &str)]) -> Block {
    let items = items
        .iter()
        .map(|(value, label)| Stat {
            value: (*value).into(),
            label: (*label).into(),
        })
        .collect();
    block("stats", 12, BlockContent::Stats(StatsProps { items }))
}

fn teams(width: u8, intro: &str, list: &[(&str, &str, &str)]) -> Block {
    let teams = list
        .iter()
        .map(|(name, about, channel)| Team {
            name: (*name).into(),
            about: (*about).into(),
            channel: (*channel).into(),
        })
        .collect();
    block(
        "teams",
        width,
        BlockContent::Teams(TeamsProps {
            heading: "Teams".into(),
            intro: intro.into(),
            teams,
        }),
    )
}

fn projects(heading: &str) -> Block {
    block(
        "projects",
        12,
        BlockContent::Projects(ProjectsProps {
            heading: heading.into(),
        }),
    )
}

fn quote(width: u8, text: &str, attribution: &str) -> Block {
    block(
        "quote",
        width,
        BlockContent::Quote(QuoteProps {
            text: text.into(),
            attribution: attribution.into(),
        }),
    )
}

fn work_sessions(width: u8) -> Block {
    block(
        "work-sessions",
        width,
        BlockContent::WorkSessions(WorkSessionsProps {}),
    )
}

fn cta(heading: &str, text: &str, primary: (&str, &str), secondary: (&str, &str)) -> Block {
    let button = |(label, url): (&str, &str)| {
        Some(LinkButton {
            label: label.into(),
            url: url.into(),
        })
    };
    block(
        "cta",
        12,
        BlockContent::Cta(CtaProps {
            heading: heading.into(),
            text: text.into(),
            primary: button(primary),
            secondary: button(secondary),
        }),
    )
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::blocks;
    use crate::testing;

    #[test]
    fn every_seeded_page_is_clean_and_ready_to_publish() {
        let images = SeedImages::load().unwrap();
        for committee in Committee::ALL {
            let doc = page(*committee, &images);
            let clean =
                blocks::clean_draft(doc.clone()).unwrap_or_else(|e| panic!("{committee}: {e:?}"));
            assert_eq!(
                clean, doc,
                "{committee} is stored as the editor would save it"
            );
            blocks::check_for_publishing(&doc).unwrap_or_else(|e| panic!("{committee}: {e:?}"));
            for (field, image) in doc.images() {
                assert!(
                    image.width.is_some(),
                    "{committee} {field} has a known size"
                );
            }
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn seeds_seven_published_pages_once(db: PgPool) {
        testing::ensure_committees(&db).await;
        let mut conn = db.acquire().await.unwrap();
        run(&mut conn, &testing::config()).await.unwrap();
        let published: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM committee_pages WHERE published IS NOT NULL AND published = draft",
        )
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        assert_eq!(published, 7);

        // An edit survives seeding again.
        sqlx::query("UPDATE committee_pages SET draft = jsonb_set(draft, '{settings,teamsLine}', '\"Edited\"') WHERE committee = 'tech'")
            .execute(&mut *conn)
            .await
            .unwrap();
        run(&mut conn, &testing::config()).await.unwrap();
        let line: String = sqlx::query_scalar(
            "SELECT draft->'settings'->>'teamsLine' FROM committee_pages WHERE committee = 'tech'",
        )
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        assert_eq!(line, "Edited");
    }
}
