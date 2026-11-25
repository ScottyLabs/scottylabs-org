//! What an editor changes on a project (`ProjectDoc` in types.ts), and the rules for saving and publishing it.

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::{AppResult, Problems};
use crate::models::{
    Committee, ImageRef, MAX_ALT_LEN, check_https_url, check_line, check_video_url, text_enum,
};
use crate::richtext;

text_enum!(
    /// Shown on the card: how the project is doing.
    ProjectStatus {
        Live => "live",
        InDevelopment => "in_development",
        Annual => "annual",
        Ongoing => "ongoing",
    }
);

text_enum!(
    /// Shown on the card: what sort of thing it is.
    ProjectKind {
        App => "app",
        Event => "event",
        Tool => "tool",
        Brand => "brand",
        Program => "program",
        Fundraiser => "fundraiser",
        Other => "other",
    }
);

text_enum!(
    /// Whether visitors can see the project.
    ProjectState {
        Draft => "draft",
        Published => "published",
        Archived => "archived",
    }
);

pub const MAX_NAME_CHARS: usize = 60;
pub const MAX_SUMMARY_CHARS: usize = 48;
pub const MAX_MEDIA: usize = 8;
/// Longest card excerpt, cut from the description.
pub const EXCERPT_CHARS: usize = 240;
pub const MAX_SLUG_CHARS: usize = 60;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDoc {
    pub name: String,
    #[serde(default)]
    pub summary: String,
    /// Sanitized HTML with the rich text rules in `richtext`.
    #[serde(default)]
    pub description: String,
    pub committee: Committee,
    #[serde(default)]
    pub built_with: Vec<Committee>,
    pub status: ProjectStatus,
    pub kind: ProjectKind,
    #[serde(default)]
    pub site_url: Option<String>,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub video_url: Option<String>,
    /// The first image is the cover.
    #[serde(default)]
    pub media: Vec<ImageRef>,
}

impl ProjectDoc {
    /// The draft of a project someone just added.
    pub fn new(name: String, committee: Committee) -> Self {
        Self {
            name,
            summary: String::new(),
            description: String::new(),
            committee,
            built_with: Vec::new(),
            status: ProjectStatus::InDevelopment,
            kind: ProjectKind::App,
            site_url: None,
            repo_url: None,
            video_url: None,
            media: Vec::new(),
        }
    }

    pub fn cover(&self) -> Option<&ImageRef> {
        self.media.first()
    }

    pub fn excerpt(&self) -> String {
        richtext::excerpt(&self.description, EXCERPT_CHARS)
    }
}

/// What is still missing before publishing. Mirrors `ProjectChecklist` in types.ts.
/// Publishing needs `cover`, `summary` and `alt_text`; a description and links are recommended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checklist {
    pub cover: bool,
    pub summary: bool,
    pub description: bool,
    pub links: bool,
    pub alt_text: bool,
}

impl Checklist {
    pub fn of(doc: &ProjectDoc) -> Self {
        let summary = doc.summary.trim().chars().count();
        Self {
            cover: doc.cover().is_some(),
            summary: (1..=MAX_SUMMARY_CHARS).contains(&summary),
            description: !richtext::plain_text(&doc.description).is_empty(),
            links: doc.site_url.is_some() || doc.repo_url.is_some(),
            alt_text: doc
                .cover()
                .is_some_and(|cover| !cover.alt.trim().is_empty()),
        }
    }
}

/// Whether a check is for saving a draft or for publishing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Save,
    Publish,
}

/// Checks a doc and returns it cleaned up: text trimmed, the description sanitized, links normalized, "Built with" in committee order without the owner, and uploaded photos sized from the media table. Fails with every problem at once, keyed by field path (`media.0.alt`).
pub async fn validate(
    conn: &mut PgConnection,
    doc: ProjectDoc,
    purpose: Purpose,
) -> AppResult<ProjectDoc> {
    let mut problems = Problems::new();
    let mut clean = clean_fields(doc, &mut problems);
    size_uploads(conn, &mut clean.media, &mut problems).await?;
    if purpose == Purpose::Publish {
        check_publishable(&clean, &mut problems);
    }
    problems.finish()?;
    Ok(clean)
}

/// The checks that need no database.
pub fn clean_fields(doc: ProjectDoc, problems: &mut Problems) -> ProjectDoc {
    let name =
        check_line(&doc.name, 1, MAX_NAME_CHARS, "the project name").unwrap_or_else(|message| {
            problems.add("name", message);
            doc.name.clone()
        });
    let summary =
        check_line(&doc.summary, 0, MAX_SUMMARY_CHARS, "the summary").unwrap_or_else(|message| {
            problems.add("summary", message);
            doc.summary.clone()
        });
    let description = clean_description(&doc.description, problems);

    let mut built_with: Vec<Committee> = doc
        .built_with
        .iter()
        .copied()
        .filter(|committee| *committee != doc.committee)
        .collect();
    built_with.sort();
    built_with.dedup();

    let site_url = optional_link(
        doc.site_url.as_deref(),
        "siteUrl",
        check_https_url,
        problems,
    );
    let repo_url = optional_link(
        doc.repo_url.as_deref(),
        "repoUrl",
        check_https_url,
        problems,
    );
    let video_url = optional_link(
        doc.video_url.as_deref(),
        "videoUrl",
        check_video_url,
        problems,
    );
    let media = clean_media(doc.media, problems);

    ProjectDoc {
        name,
        summary,
        description,
        committee: doc.committee,
        built_with,
        status: doc.status,
        kind: doc.kind,
        site_url,
        repo_url,
        video_url,
        media,
    }
}

fn clean_description(raw: &str, problems: &mut Problems) -> String {
    let html = richtext::sanitize(raw);
    let length = richtext::plain_text(&html).chars().count();
    if length == 0 {
        // Empty paragraphs and line breaks count as no description.
        return String::new();
    }
    if length > richtext::MAX_TEXT_CHARS {
        problems.add(
            "description",
            format!(
                "Shorten the description to {} characters or fewer. It has {length} now.",
                group_thousands(richtext::MAX_TEXT_CHARS)
            ),
        );
    }
    html
}

/// "4,000" for 4000.
fn group_thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `None` for a missing or blank link; otherwise the checked link.
fn optional_link(
    raw: Option<&str>,
    field: &str,
    check: fn(&str) -> Result<String, String>,
    problems: &mut Problems,
) -> Option<String> {
    let raw = raw.map(str::trim).filter(|value| !value.is_empty())?;
    match check(raw) {
        Ok(link) => Some(link),
        Err(message) => {
            problems.add(field, message);
            Some(raw.to_owned())
        }
    }
}

fn clean_media(media: Vec<ImageRef>, problems: &mut Problems) -> Vec<ImageRef> {
    if media.len() > MAX_MEDIA {
        problems.add(
            "media",
            format!("Use {MAX_MEDIA} photos or fewer. Remove one to add another."),
        );
    }
    media
        .into_iter()
        .enumerate()
        .map(|(index, image)| {
            let image = ImageRef {
                alt: image.alt.trim().to_owned(),
                ..image
            };
            if image.alt.chars().count() > MAX_ALT_LEN {
                problems.add(
                    format!("media.{index}.alt"),
                    format!("Keep the photo description to {MAX_ALT_LEN} characters or fewer."),
                );
            } else if let Err(message) = image.check(false) {
                problems.add(format!("media.{index}"), message);
            }
            image
        })
        .collect()
}

/// Uploaded photos must still exist. Their size comes from the media table, not from the request.
async fn size_uploads(
    conn: &mut PgConnection,
    media: &mut [ImageRef],
    problems: &mut Problems,
) -> AppResult<()> {
    let ids: Vec<Uuid> = media.iter().filter_map(ImageRef::media_id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let stored: Vec<(Uuid, i32, i32)> =
        sqlx::query_as("SELECT id, width, height FROM media WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(conn)
            .await?;
    for (index, image) in media.iter_mut().enumerate() {
        let Some(id) = image.media_id() else {
            continue;
        };
        match stored.iter().find(|(stored_id, _, _)| *stored_id == id) {
            Some((_, width, height)) => {
                image.width = u32::try_from(*width).ok();
                image.height = u32::try_from(*height).ok();
            }
            None => problems.add(
                format!("media.{index}"),
                "That photo is no longer available. Remove it and upload it again.",
            ),
        }
    }
    Ok(())
}

/// The extra rules for publishing: a cover with alt text, and a summary.
fn check_publishable(doc: &ProjectDoc, problems: &mut Problems) {
    match doc.cover() {
        None => problems.add(
            "media",
            "Add at least one photo before publishing. The first one is the cover.",
        ),
        Some(cover) if cover.alt.is_empty() => problems.add(
            "media.0.alt",
            "Describe the cover photo for people who use screen readers.",
        ),
        Some(_) => {}
    }
    if doc.summary.is_empty() {
        problems.add(
            "summary",
            "Write a one-line summary before publishing. It appears on the project card.",
        );
    }
}

/// A project address made from its name: lowercase letters and numbers joined by single hyphens.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_hyphen = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_hyphen && !slug.is_empty() {
                slug.push('-');
            }
            pending_hyphen = false;
            slug.push(c.to_ascii_lowercase());
        } else if !matches!(c, '\'' | '\u{2019}') {
            // Apostrophes join words ("Sara's" becomes "saras"); anything else separates them.
            pending_hyphen = true;
        }
    }
    slug.truncate(MAX_SLUG_CHARS);
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "project".to_owned()
    } else {
        slug.to_owned()
    }
}

/// Checks an address an admin typed. Returns it lowercased and trimmed.
pub fn check_slug(raw: &str) -> Result<String, String> {
    let slug = raw.trim().trim_matches('/').to_ascii_lowercase();
    let slug = slug.strip_prefix("projects/").unwrap_or(&slug).to_owned();
    let valid = !slug.is_empty()
        && slug.len() <= MAX_SLUG_CHARS
        && slug.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
    if valid {
        Ok(slug)
    } else {
        Err(format!(
            "Use lowercase letters, numbers and single hyphens, up to {MAX_SLUG_CHARS} characters, like cmu-maps."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> ProjectDoc {
        ProjectDoc {
            summary: "Room-level navigation of campus.".into(),
            media: vec![ImageRef {
                src: "/seed/projects/maps.jpg".into(),
                alt: "A map".into(),
                width: Some(1440),
                height: Some(900),
            }],
            ..ProjectDoc::new("CMU Maps".into(), Committee::Tech)
        }
    }

    fn fields(doc: ProjectDoc) -> (ProjectDoc, Vec<String>) {
        let mut problems = Problems::new();
        let clean = clean_fields(doc, &mut problems);
        let keys = match problems.finish() {
            Ok(()) => Vec::new(),
            Err(crate::error::AppError::Validation { fields, .. }) => fields.into_keys().collect(),
            Err(other) => panic!("unexpected {other:?}"),
        };
        (clean, keys)
    }

    #[test]
    fn slugs_come_from_names() {
        assert_eq!(slugify("CMU Maps"), "cmu-maps");
        assert_eq!(slugify("  TartanHacks identity! "), "tartanhacks-identity");
        assert_eq!(
            slugify("ScottyLabs\u{2019} fall hackathon"),
            "scottylabs-fall-hackathon"
        );
        assert_eq!(slugify("Sara's app"), "saras-app");
        assert_eq!(slugify("\u{1f600}\u{1f600}"), "project");
        assert_eq!(slugify("caf\u{e9} 2.0"), "caf-2-0");
        let long = slugify(&"word ".repeat(40));
        assert!(long.len() <= MAX_SLUG_CHARS);
        assert!(!long.ends_with('-'));
        assert!(check_slug(&long).is_ok());
    }

    #[test]
    fn admins_type_addresses_in_the_slug_shape() {
        assert_eq!(check_slug(" CMU-Maps ").unwrap(), "cmu-maps");
        assert_eq!(check_slug("/projects/cmu-maps/").unwrap(), "cmu-maps");
        for bad in [
            "",
            "cmu maps",
            "cmu--maps",
            "-maps",
            "maps-",
            "../x",
            "a/b",
            "\u{e9}t\u{e9}",
        ] {
            assert!(check_slug(bad).is_err(), "{bad:?}");
        }
        assert!(check_slug(&"a".repeat(61)).is_err());
    }

    #[test]
    fn saving_trims_sanitizes_and_normalizes() {
        let (clean, problems) = fields(ProjectDoc {
            name: "  CMU Maps  ".into(),
            description:
                r#"<p onclick="x()">Hi <script>alert(1)</script><strong>there</strong></p>"#.into(),
            built_with: vec![
                Committee::Design,
                Committee::Tech,
                Committee::Events,
                Committee::Design,
            ],
            site_url: Some("   ".into()),
            repo_url: Some(" https://git.cmu.dev/ScottyLabs/maps ".into()),
            ..doc()
        });
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(clean.name, "CMU Maps");
        assert!(!clean.description.contains("script"));
        assert!(!clean.description.contains("onclick"));
        assert!(clean.description.contains("<strong>there</strong>"));
        assert_eq!(clean.built_with, vec![Committee::Events, Committee::Design]);
        assert_eq!(clean.site_url, None);
        assert_eq!(
            clean.repo_url.as_deref(),
            Some("https://git.cmu.dev/ScottyLabs/maps")
        );
    }

    #[test]
    fn empty_descriptions_are_empty() {
        let (clean, _) = fields(ProjectDoc {
            description: "<p></p><p><br></p>".into(),
            ..doc()
        });
        assert_eq!(clean.description, "");
    }

    #[test]
    fn every_problem_has_a_field() {
        let (_, problems) = fields(ProjectDoc {
            name: String::new(),
            summary: "x".repeat(49),
            description: format!("<p>{}</p>", "a".repeat(4001)),
            site_url: Some("http://example.com".into()),
            repo_url: Some("javascript:alert(1)".into()),
            video_url: Some("https://example.com/video.mp4".into()),
            media: vec![
                ImageRef {
                    src: "https://evil.example/x.png".into(),
                    alt: String::new(),
                    width: None,
                    height: None,
                },
                ImageRef {
                    src: "/seed/projects/maps.jpg".into(),
                    alt: "a".repeat(301),
                    width: None,
                    height: None,
                },
            ],
            ..doc()
        });
        for field in [
            "name",
            "summary",
            "description",
            "siteUrl",
            "repoUrl",
            "videoUrl",
            "media.0",
            "media.1.alt",
        ] {
            assert!(
                problems.iter().any(|p| p == field),
                "{field} in {problems:?}"
            );
        }
    }

    #[test]
    fn at_most_eight_photos() {
        let many = vec![doc().media[0].clone(); MAX_MEDIA + 1];
        let (_, problems) = fields(ProjectDoc {
            media: many,
            ..doc()
        });
        assert_eq!(problems, vec!["media".to_owned()]);
    }

    #[test]
    fn publishing_needs_a_described_cover_and_a_summary() {
        let mut problems = Problems::new();
        check_publishable(&doc(), &mut problems);
        assert!(problems.finish().is_ok());

        let mut problems = Problems::new();
        check_publishable(
            &ProjectDoc {
                summary: String::new(),
                media: Vec::new(),
                ..doc()
            },
            &mut problems,
        );
        let Err(crate::error::AppError::Validation { fields, .. }) = problems.finish() else {
            panic!("expected problems");
        };
        assert!(fields.contains_key("media") && fields.contains_key("summary"));

        let mut undescribed = doc();
        undescribed.media[0].alt = String::new();
        let mut problems = Problems::new();
        check_publishable(&undescribed, &mut problems);
        let Err(crate::error::AppError::Validation { fields, .. }) = problems.finish() else {
            panic!("expected problems");
        };
        assert!(fields.contains_key("media.0.alt"));
    }

    #[test]
    fn the_checklist_follows_the_doc() {
        let list = Checklist::of(&doc());
        assert!(list.cover && list.summary && list.alt_text);
        assert!(!list.description && !list.links);
        let list = Checklist::of(&ProjectDoc::new("New".into(), Committee::Tech));
        assert!(!list.cover && !list.summary && !list.alt_text);
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(4000), "4,000");
        assert_eq!(group_thousands(48), "48");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
    }
}
