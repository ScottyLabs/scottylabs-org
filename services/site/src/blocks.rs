//! Committee page documents: the blocks a page is built from, the checks for saving a draft and for publishing it, and plain sentences that say what changed since the last published version.
//!
//! Mirrors `CommitteePageDoc` and `Block` in sites/web/src/lib/api/types.ts. Validation errors use field paths `blocks.<index>.<prop>` (for example `blocks.3.items.1.value` or `blocks.0.image.alt`) and `settings.<field>`, so the editor can point at the block that needs work. The limits are in [`limits`] and mirrored in sites/web/src/lib/committees/limits.ts.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, Problems};
use crate::models::{ImageRef, check_button_url, check_https_url, check_line, check_video_url};
use crate::richtext;

/// A page has at most this many blocks, the cover included.
pub const MAX_BLOCKS: usize = 12;

/// Longest text in each field, counted in characters after trimming.
pub mod limits {
    pub const TAGLINE: usize = 120;
    pub const HEADING: usize = 80;
    pub const CAPTION: usize = 200;
    /// Quotes, the call to action text and the teams introduction.
    pub const SHORT_TEXT: usize = 240;
    pub const STAT_VALUE: usize = 12;
    pub const STAT_LABEL: usize = 60;
    pub const TEAM_NAME: usize = 40;
    pub const TEAM_ABOUT: usize = 140;
    pub const ATTRIBUTION: usize = 80;
    pub const BUTTON_LABEL: usize = 40;
    pub const VIDEO_TITLE: usize = 80;
    pub const INDEX_DESCRIPTION: usize = 300;
    pub const TEAMS_LINE: usize = 120;
    pub const MEETING_TIME: usize = 60;
    pub const MEETING_ROOM: usize = 80;
    /// Slack's own limit for channel names.
    pub const CHANNEL: usize = 80;
    pub const MAX_TEAMS: usize = 20;
    pub const MIN_STATS: usize = 2;
    pub const MAX_STATS: usize = 4;
    pub const MIN_GALLERY: usize = 3;
    pub const MAX_GALLERY: usize = 6;
    /// Stored rich text, in bytes of HTML. Keeps heavily formatted text from growing without bound.
    pub const BODY_HTML_BYTES: usize = 32 * 1024;
    pub const BLOCK_ID: usize = 64;
}

// ---------------------------------------------------------------------------
// The document

/// A committee page: its blocks in order and its settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitteePageDoc {
    pub blocks: Vec<Block>,
    pub settings: CommitteeSettings,
}

/// Mirrors `CommitteeSettings` in types.ts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommitteeSettings {
    pub index_description: String,
    pub teams_line: String,
    pub meeting_time: String,
    pub meeting_room: String,
    pub map_url: Option<String>,
    /// Without the leading `#`.
    pub slack_channel: Option<String>,
}

/// One block: `{ id, type, width, props }` on the wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub width: u8,
    #[serde(flatten)]
    pub content: BlockContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "props", rename_all = "snake_case")]
pub enum BlockContent {
    Cover(CoverProps),
    Text(TextProps),
    Image(ImageProps),
    Gallery(GalleryProps),
    Video(VideoProps),
    Stats(StatsProps),
    Teams(TeamsProps),
    Projects(ProjectsProps),
    Quote(QuoteProps),
    WorkSessions(WorkSessionsProps),
    Cta(CtaProps),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoverProps {
    pub tagline: String,
    pub image: Option<ImageRef>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextProps {
    pub heading: String,
    /// Sanitized rich text.
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageProps {
    pub image: Option<ImageRef>,
    pub caption: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryLayout {
    #[default]
    Mosaic,
    Thirds,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GalleryProps {
    pub images: Vec<ImageRef>,
    pub layout: GalleryLayout,
    pub caption: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoProps {
    /// A YouTube or Vimeo link. May be empty in a draft.
    pub url: String,
    pub title: String,
    pub poster: Option<ImageRef>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsProps {
    pub items: Vec<Stat>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stat {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TeamsProps {
    pub heading: String,
    pub intro: String,
    pub teams: Vec<Team>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Team {
    pub name: String,
    pub about: String,
    /// A Slack channel without the leading `#`, or "".
    pub channel: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectsProps {
    pub heading: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuoteProps {
    /// Without quotation marks; the page adds them.
    pub text: String,
    pub attribution: String,
}

/// The work sessions block has no props of its own: it shows the committee settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSessionsProps {}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CtaProps {
    pub heading: String,
    pub text: String,
    pub primary: Option<LinkButton>,
    pub secondary: Option<LinkButton>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkButton {
    pub label: String,
    pub url: String,
}

impl BlockContent {
    /// The name people see in the block library and the page outline.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cover(_) => "Cover",
            Self::Text(_) => "Text",
            Self::Image(_) => "Image",
            Self::Gallery(_) => "Gallery",
            Self::Video(_) => "Video",
            Self::Stats(_) => "Stats",
            Self::Teams(_) => "Teams",
            Self::Projects(_) => "Projects",
            Self::Quote(_) => "Quote",
            Self::WorkSessions(_) => "Work sessions",
            Self::Cta(_) => "Call to action",
        }
    }

    /// Widths in columns of the 12-column grid. Mirrors `BLOCK_WIDTHS` in types.ts.
    pub fn widths(&self) -> &'static [u8] {
        match self {
            Self::Cover(_) | Self::Projects(_) | Self::Cta(_) => &[12],
            Self::Text(_) | Self::Image(_) | Self::Quote(_) => &[4, 6, 8, 12],
            Self::Gallery(_) | Self::Teams(_) => &[8, 12],
            Self::Video(_) | Self::Stats(_) => &[6, 8, 12],
            Self::WorkSessions(_) => &[4, 6],
        }
    }

    pub fn is_cover(&self) -> bool {
        matches!(self, Self::Cover(_))
    }
}

impl Block {
    /// How sentences name the block: a text block by its heading, anything else by its type.
    pub fn name(&self) -> String {
        match &self.content {
            BlockContent::Text(text) if !text.heading.trim().is_empty() => {
                text.heading.trim().to_owned()
            }
            other => other.label().to_owned(),
        }
    }
}

impl CommitteePageDoc {
    /// The cover block's props, if the page starts with one.
    pub fn cover(&self) -> Option<&CoverProps> {
        match self.blocks.first().map(|block| &block.content) {
            Some(BlockContent::Cover(cover)) => Some(cover),
            _ => None,
        }
    }

    /// Every image on the page with its field path, such as `blocks.3.images.1`.
    pub fn images(&self) -> Vec<(String, &ImageRef)> {
        let mut found = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            let at = |prop: &str| format!("blocks.{index}.{prop}");
            match &block.content {
                BlockContent::Cover(CoverProps {
                    image: Some(image), ..
                })
                | BlockContent::Image(ImageProps {
                    image: Some(image), ..
                }) => found.push((at("image"), image)),
                BlockContent::Video(VideoProps {
                    poster: Some(image),
                    ..
                }) => found.push((at("poster"), image)),
                BlockContent::Gallery(gallery) => {
                    for (position, image) in gallery.images.iter().enumerate() {
                        found.push((at(&format!("images.{position}")), image));
                    }
                }
                _ => {}
            }
        }
        found
    }

    /// Every image on the page, for filling in stored sizes.
    pub fn images_mut(&mut self) -> Vec<&mut ImageRef> {
        let mut found = Vec::new();
        for block in &mut self.blocks {
            match &mut block.content {
                BlockContent::Cover(CoverProps {
                    image: Some(image), ..
                })
                | BlockContent::Image(ImageProps {
                    image: Some(image), ..
                })
                | BlockContent::Video(VideoProps {
                    poster: Some(image),
                    ..
                }) => found.push(image),
                BlockContent::Gallery(gallery) => found.extend(gallery.images.iter_mut()),
                _ => {}
            }
        }
        found
    }

    /// Ids of uploaded images on the page, without duplicates.
    pub fn media_ids(&self) -> Vec<Uuid> {
        let mut ids = Vec::new();
        for (_, image) in self.images() {
            if let Some(id) = image.media_id()
                && !ids.contains(&id)
            {
                ids.push(id);
            }
        }
        ids
    }
}

// ---------------------------------------------------------------------------
// Checks for saving

/// Checks a draft for saving and returns it cleaned: text trimmed, rich text sanitized, links normalized and Slack channels without `#`. Fails with every problem found, keyed by field path.
pub fn clean_draft(mut doc: CommitteePageDoc) -> AppResult<CommitteePageDoc> {
    let mut problems = Problems::new();
    check_structure(&doc.blocks, &mut problems);
    for (index, block) in doc.blocks.iter_mut().enumerate() {
        let mut fields = Fields {
            prefix: format!("blocks.{index}"),
            problems: &mut problems,
        };
        clean_block(&mut block.content, &mut fields);
    }
    clean_settings(&mut doc.settings, &mut problems);
    problems.finish()?;
    Ok(doc)
}

/// Collects problems under one prefix, such as `blocks.3`.
struct Fields<'a> {
    prefix: String,
    problems: &'a mut Problems,
}

impl Fields<'_> {
    fn add(&mut self, prop: &str, message: impl Into<String>) {
        let field = if prop.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}.{prop}", self.prefix)
        };
        self.problems.add(field, message);
    }

    /// Trims a one-line text field that may be empty and keeps it within `max` characters.
    fn line(&mut self, prop: &str, value: &mut String, max: usize, label: &str) {
        match check_line(value.as_str(), 0, max, label) {
            Ok(clean) => *value = clean,
            Err(message) => self.add(prop, message),
        }
    }

    fn image(&mut self, prop: &str, image: &ImageRef) {
        if let Err(message) = image.check(false) {
            self.add(prop, message);
        }
    }
}

fn check_structure(blocks: &[Block], problems: &mut Problems) {
    if blocks.len() > MAX_BLOCKS {
        problems.add(
            "blocks",
            format!(
                "A page can have at most {MAX_BLOCKS} blocks, including the cover. Remove one to add another."
            ),
        );
    }
    if !blocks.first().is_some_and(|block| block.content.is_cover()) {
        problems.add("blocks", "Start the page with its cover.");
    }
    let mut seen = HashSet::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 && block.content.is_cover() {
            problems.add(
                format!("blocks.{index}"),
                "A page has one cover, at the top. Remove this one.",
            );
        }
        if !is_valid_block_id(&block.id) || !seen.insert(block.id.as_str()) {
            problems.add(
                format!("blocks.{index}.id"),
                "Each block needs its own id. Reload the page and try again.",
            );
        }
        if !block.content.widths().contains(&block.width) {
            problems.add(
                format!("blocks.{index}.width"),
                width_message(block.content.widths()),
            );
        }
    }
}

fn is_valid_block_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= limits::BLOCK_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn width_message(widths: &[u8]) -> String {
    if let [only] = widths {
        return format!("This block is always {}.", width_phrase(*only));
    }
    let names: Vec<&str> = widths.iter().map(|w| width_name(*w)).collect();
    let list = match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => names.join(""),
    };
    format!("Make this block {list} width.")
}

fn width_name(width: u8) -> &'static str {
    match width {
        4 => "one third",
        6 => "half",
        8 => "two thirds",
        _ => "full",
    }
}

fn width_phrase(width: u8) -> String {
    format!("{} width", width_name(width))
}

/// The video a YouTube or Vimeo link plays, read the way the page reads it (`videoEmbed` in sites/web/src/lib/committees/video.ts): YouTube watch, `youtu.be`, embed, shorts and live links, and Vimeo links with a video number. Channels, playlists and profiles play no single video, so the page could not show them.
pub fn video_id(link: &str) -> Option<String> {
    let url = url::Url::parse(link.trim()).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?;
    let host = host
        .strip_prefix("www.")
        .or_else(|| host.strip_prefix("m."))
        .unwrap_or(host);
    let parts: Vec<&str> = url.path().split('/').collect();
    match host {
        "youtube.com" | "youtu.be" => {
            let id = if host == "youtu.be" {
                parts.get(1).map(|id| (*id).to_owned())
            } else if url.path() == "/watch" {
                url.query_pairs()
                    .find(|(key, _)| key == "v")
                    .map(|(_, id)| id.into_owned())
            } else {
                match (parts.get(1), parts.get(2)) {
                    (Some(&("embed" | "shorts" | "live")), Some(id)) => Some((*id).to_owned()),
                    _ => None,
                }
            };
            id.filter(|id| {
                (6..=20).contains(&id.len())
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
        }
        "vimeo.com" | "player.vimeo.com" => {
            let is_number =
                |id: &&str| (3..=12).contains(&id.len()) && id.chars().all(|c| c.is_ascii_digit());
            let named: Vec<&str> = parts.into_iter().filter(|part| !part.is_empty()).collect();
            let id = if host == "player.vimeo.com" {
                (named.first() == Some(&"video"))
                    .then(|| named.get(1).copied())
                    .flatten()
                    .filter(is_number)
            } else {
                named.into_iter().find(is_number)
            };
            id.map(str::to_owned)
        }
        _ => None,
    }
}

fn clean_block(content: &mut BlockContent, fields: &mut Fields<'_>) {
    match content {
        BlockContent::Cover(cover) => {
            fields.line(
                "tagline",
                &mut cover.tagline,
                limits::TAGLINE,
                "the tagline",
            );
            if let Some(image) = &cover.image {
                fields.image("image", image);
            }
        }
        BlockContent::Text(text) => {
            fields.line("heading", &mut text.heading, limits::HEADING, "the heading");
            clean_rich_text(&mut text.body, fields);
        }
        BlockContent::Image(image) => {
            if let Some(picture) = &image.image {
                fields.image("image", picture);
            }
            fields.line(
                "caption",
                &mut image.caption,
                limits::CAPTION,
                "the caption",
            );
        }
        BlockContent::Gallery(gallery) => {
            if gallery.images.len() > limits::MAX_GALLERY {
                fields.add(
                    "images",
                    format!(
                        "A gallery holds at most {} photos. Remove one to add another.",
                        limits::MAX_GALLERY
                    ),
                );
            }
            for (position, image) in gallery.images.iter().enumerate() {
                fields.image(&format!("images.{position}"), image);
            }
            fields.line(
                "caption",
                &mut gallery.caption,
                limits::CAPTION,
                "the caption",
            );
        }
        BlockContent::Video(video) => {
            let url = video.url.trim();
            if url.is_empty() {
                video.url.clear();
            } else {
                match check_video_url(url) {
                    Ok(clean) if video_id(&clean).is_some() => video.url = clean,
                    Ok(_) => fields.add(
                        "url",
                        "Use a link to one YouTube or Vimeo video, not a channel or playlist.",
                    ),
                    Err(message) => fields.add("url", message),
                }
            }
            fields.line("title", &mut video.title, limits::VIDEO_TITLE, "the title");
            if let Some(poster) = &video.poster {
                fields.image("poster", poster);
            }
        }
        BlockContent::Stats(stats) => clean_stats(stats, fields),
        BlockContent::Teams(teams) => clean_teams(teams, fields),
        BlockContent::Projects(projects) => {
            fields.line(
                "heading",
                &mut projects.heading,
                limits::HEADING,
                "the heading",
            );
        }
        BlockContent::Quote(quote) => {
            fields.line("text", &mut quote.text, limits::SHORT_TEXT, "the quote");
            fields.line(
                "attribution",
                &mut quote.attribution,
                limits::ATTRIBUTION,
                "who said it",
            );
        }
        BlockContent::WorkSessions(_) => {}
        BlockContent::Cta(cta) => {
            fields.line("heading", &mut cta.heading, limits::HEADING, "the heading");
            fields.line("text", &mut cta.text, limits::SHORT_TEXT, "the text");
            clean_button("primary", &mut cta.primary, fields);
            clean_button("secondary", &mut cta.secondary, fields);
        }
    }
}

fn clean_rich_text(body: &mut String, fields: &mut Fields<'_>) {
    const TOO_MUCH: &str =
        "This text has too much formatting to save. Remove some of it and try again.";
    let Ok(clean) = richtext::clean(body) else {
        fields.add("body", TOO_MUCH);
        return;
    };
    if clean.len() > limits::BODY_HTML_BYTES {
        fields.add("body", TOO_MUCH);
        return;
    }
    if richtext::plain_text(&clean).chars().count() > richtext::MAX_TEXT_CHARS {
        fields.add(
            "body",
            format!(
                "Shorten this text to {} characters or fewer.",
                richtext::MAX_TEXT_CHARS
            ),
        );
    }
    // Text that is only empty paragraphs is no text.
    *body = if richtext::plain_text(&clean).is_empty() {
        String::new()
    } else {
        clean
    };
}

fn clean_stats(stats: &mut StatsProps, fields: &mut Fields<'_>) {
    if !(limits::MIN_STATS..=limits::MAX_STATS).contains(&stats.items.len()) {
        fields.add(
            "items",
            format!(
                "Show between {} and {} numbers.",
                limits::MIN_STATS,
                limits::MAX_STATS
            ),
        );
    }
    for (position, item) in stats.items.iter_mut().enumerate() {
        fields.line(
            &format!("items.{position}.value"),
            &mut item.value,
            limits::STAT_VALUE,
            "the number",
        );
        fields.line(
            &format!("items.{position}.label"),
            &mut item.label,
            limits::STAT_LABEL,
            "the label",
        );
    }
}

fn clean_teams(teams: &mut TeamsProps, fields: &mut Fields<'_>) {
    fields.line(
        "heading",
        &mut teams.heading,
        limits::HEADING,
        "the heading",
    );
    fields.line(
        "intro",
        &mut teams.intro,
        limits::SHORT_TEXT,
        "the introduction",
    );
    if teams.teams.len() > limits::MAX_TEAMS {
        fields.add(
            "teams",
            format!("List at most {} teams.", limits::MAX_TEAMS),
        );
    }
    for (position, team) in teams.teams.iter_mut().enumerate() {
        fields.line(
            &format!("teams.{position}.name"),
            &mut team.name,
            limits::TEAM_NAME,
            "the team name",
        );
        fields.line(
            &format!("teams.{position}.about"),
            &mut team.about,
            limits::TEAM_ABOUT,
            "the description",
        );
        match clean_channel(&team.channel) {
            Ok(channel) => team.channel = channel.unwrap_or_default(),
            Err(message) => fields.add(&format!("teams.{position}.channel"), message),
        }
    }
}

fn clean_button(prop: &str, button: &mut Option<LinkButton>, fields: &mut Fields<'_>) {
    let Some(link) = button else {
        return;
    };
    let label = link.label.trim();
    let url = link.url.trim();
    if label.is_empty() && url.is_empty() {
        *button = None;
        return;
    }
    match check_line(label, 1, limits::BUTTON_LABEL, "the button label") {
        Ok(clean) => link.label = clean,
        Err(message) => fields.add(&format!("{prop}.label"), message),
    }
    if url.is_empty() {
        fields.add(&format!("{prop}.url"), "Add the link this button opens.");
        return;
    }
    match check_button_url(url) {
        Ok(clean) => link.url = clean,
        Err(message) => fields.add(&format!("{prop}.url"), message),
    }
}

/// A Slack channel name without `#`, or `None` when empty.
fn clean_channel(raw: &str) -> Result<Option<String>, String> {
    let name = raw.trim().trim_start_matches('#').to_ascii_lowercase();
    if name.is_empty() {
        return Ok(None);
    }
    let valid = name.len() <= limits::CHANNEL
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'));
    if valid {
        Ok(Some(name))
    } else {
        Err(
            "Enter a Slack channel name such as tech-maps, with letters, numbers and dashes."
                .into(),
        )
    }
}

fn clean_settings(settings: &mut CommitteeSettings, problems: &mut Problems) {
    let mut fields = Fields {
        prefix: "settings".into(),
        problems,
    };
    fields.line(
        "indexDescription",
        &mut settings.index_description,
        limits::INDEX_DESCRIPTION,
        "the description",
    );
    fields.line(
        "teamsLine",
        &mut settings.teams_line,
        limits::TEAMS_LINE,
        "the teams line",
    );
    fields.line(
        "meetingTime",
        &mut settings.meeting_time,
        limits::MEETING_TIME,
        "the meeting time",
    );
    fields.line(
        "meetingRoom",
        &mut settings.meeting_room,
        limits::MEETING_ROOM,
        "the room",
    );
    settings.map_url = match settings.map_url.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(url) => match check_https_url(url) {
            Ok(clean) => Some(clean),
            Err(message) => {
                fields.add("mapUrl", message);
                Some(url.to_owned())
            }
        },
    };
    match clean_channel(settings.slack_channel.as_deref().unwrap_or_default()) {
        Ok(channel) => settings.slack_channel = channel,
        Err(message) => fields.add("slackChannel", message),
    }
}

// ---------------------------------------------------------------------------
// Checks for publishing

/// The extra checks before a cleaned draft goes live: a cover photo, alt text on every image, and blocks that would otherwise show up empty.
pub fn check_for_publishing(doc: &CommitteePageDoc) -> AppResult<()> {
    let mut problems = Problems::new();
    if doc.cover().is_some_and(|cover| cover.image.is_none()) {
        problems.add("blocks.0.image", "Add a cover photo before you publish.");
    }
    for (index, block) in doc.blocks.iter().enumerate() {
        let mut fields = Fields {
            prefix: format!("blocks.{index}"),
            problems: &mut problems,
        };
        check_block_for_publishing(&block.content, &mut fields);
    }
    for (field, image) in doc.images() {
        if let Err(message) = image.check(true) {
            problems.add(format!("{field}.alt"), message);
        }
    }
    problems.finish()
}

fn check_block_for_publishing(content: &BlockContent, fields: &mut Fields<'_>) {
    match content {
        BlockContent::Image(image) if image.image.is_none() => {
            fields.add(
                "image",
                "Choose a photo for this block, or remove the block.",
            );
        }
        BlockContent::Gallery(gallery) if gallery.images.len() < limits::MIN_GALLERY => {
            fields.add(
                "images",
                format!(
                    "Add at least {} photos to the gallery, or remove it.",
                    limits::MIN_GALLERY
                ),
            );
        }
        BlockContent::Video(video) if video.url.is_empty() => {
            fields.add("url", "Add a YouTube or Vimeo link, or remove the video.");
        }
        BlockContent::Teams(teams) if teams.teams.is_empty() => {
            fields.add("teams", "Add at least one team, or remove the block.");
        }
        BlockContent::Stats(stats) => {
            for (position, item) in stats.items.iter().enumerate() {
                if item.value.is_empty() {
                    fields.add(
                        &format!("items.{position}.value"),
                        "Fill in this number, or remove it.",
                    );
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// What changed

/// Plain sentences that say how the draft differs from the published page, in the order of the draft: "Added an image block", "Moved Stats above What we do", "Made What we do full width", "Edited Teams", then removed blocks and "Changed the page settings".
pub fn describe_changes(
    published: Option<&CommitteePageDoc>,
    draft: &CommitteePageDoc,
) -> Vec<String> {
    let Some(published) = published else {
        return vec!["This page has not been published yet, so all of it is new.".into()];
    };
    let old_positions: HashMap<&str, usize> = published
        .blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id.as_str(), position))
        .collect();
    // Blocks on both versions, in draft order, with their published position.
    let common: Vec<(usize, usize)> = draft
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(position, block)| {
            old_positions
                .get(block.id.as_str())
                .map(|old| (position, *old))
        })
        .collect();
    let kept_in_order = longest_increasing(&common.iter().map(|(_, old)| *old).collect::<Vec<_>>());

    let mut changes = Vec::new();
    for (position, block) in draft.blocks.iter().enumerate() {
        let Some(&old) = old_positions.get(block.id.as_str()) else {
            changes.push(format!(
                "Added {} block",
                with_article(block.content.label())
            ));
            continue;
        };
        if let Some(index) = common.iter().position(|(p, _)| *p == position)
            && !kept_in_order.contains(&index)
        {
            changes.push(move_sentence(draft, &common, index));
        }
        let before = &published.blocks[old];
        if before.width != block.width {
            changes.push(format!(
                "Made {} {}",
                block.name(),
                width_phrase(block.width)
            ));
        }
        if before.content != block.content {
            changes.push(if block.content.is_cover() {
                "Edited the cover".to_owned()
            } else {
                format!("Edited {}", block.name())
            });
        }
    }
    let draft_ids: HashSet<&str> = draft.blocks.iter().map(|block| block.id.as_str()).collect();
    for block in &published.blocks {
        if !draft_ids.contains(block.id.as_str()) {
            changes.push(format!("Removed {}", block.name()));
        }
    }
    if published.settings != draft.settings {
        changes.push("Changed the page settings".into());
    }
    changes
}

/// "Moved Stats above What we do", naming the next block that stayed, or the one before it.
fn move_sentence(draft: &CommitteePageDoc, common: &[(usize, usize)], index: usize) -> String {
    let name = draft.blocks[common[index].0].name();
    if let Some((next, _)) = common.get(index + 1) {
        return format!("Moved {name} above {}", draft.blocks[*next].name());
    }
    match index
        .checked_sub(1)
        .and_then(|previous| common.get(previous))
    {
        Some((previous, _)) => format!("Moved {name} below {}", draft.blocks[*previous].name()),
        None => format!("Moved {name}"),
    }
}

/// Indexes into `values` of one longest strictly increasing subsequence: the blocks that kept their order. Pages are small, so the simple quadratic method is enough.
fn longest_increasing(values: &[usize]) -> HashSet<usize> {
    let mut length = vec![1usize; values.len()];
    let mut previous: Vec<Option<usize>> = vec![None; values.len()];
    for i in 0..values.len() {
        for j in 0..i {
            if values[j] < values[i] && length[j] + 1 > length[i] {
                length[i] = length[j] + 1;
                previous[i] = Some(j);
            }
        }
    }
    let mut kept = HashSet::new();
    let mut cursor = (0..values.len()).max_by_key(|&i| (length[i], std::cmp::Reverse(i)));
    while let Some(i) = cursor {
        kept.insert(i);
        cursor = previous[i];
    }
    kept
}

fn with_article(label: &str) -> String {
    let lower = label.to_lowercase();
    let article = if lower.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {lower}")
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::error::AppError;

    fn image(src: &str, alt: &str) -> Value {
        json!({ "src": src, "alt": alt, "width": 1000, "height": 667 })
    }

    fn sample() -> Value {
        json!({
            "blocks": [
                { "id": "cover", "type": "cover", "width": 12,
                  "props": { "tagline": "We build the apps CMU opens every day.", "image": image("/seed/photos/c10.jpg", "Students") } },
                { "id": "what", "type": "text", "width": 8,
                  "props": { "heading": "What we do", "body": "<p>Tech is where ScottyLabs builds.</p>" } },
                { "id": "sessions", "type": "work_sessions", "width": 4, "props": {} },
                { "id": "stats", "type": "stats", "width": 12,
                  "props": { "items": [{ "value": "15", "label": "project teams" }, { "value": "2011", "label": "since" }] } },
                { "id": "teams", "type": "teams", "width": 12,
                  "props": { "heading": "Teams", "intro": "Fifteen teams.", "teams": [{ "name": "CMU Maps", "about": "Maps.", "channel": "#Tech-Maps" }] } },
                { "id": "cta", "type": "cta", "width": 12,
                  "props": { "heading": "Build with Tech", "text": "Come by.",
                             "primary": { "label": "Apply to Tech", "url": "https://tartanconnect.cmu.edu/scottylabs/club_signup" },
                             "secondary": { "label": "See sponsors", "url": "/sponsors/" } } }
            ],
            "settings": {
                "indexDescription": "Builds apps.", "teamsLine": "15 teams", "meetingTime": "Saturdays, 4\u{2013}6 PM",
                "meetingRoom": "Tepper 2700", "mapUrl": "https://maps.scottylabs.org/TEP-2700", "slackChannel": "#tech"
            }
        })
    }

    fn doc(value: Value) -> CommitteePageDoc {
        serde_json::from_value(value).unwrap()
    }

    fn fields(result: AppResult<impl std::fmt::Debug>) -> Vec<String> {
        match result {
            Err(AppError::Validation { fields, .. }) => fields.into_keys().collect(),
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    fn with_block(index: usize, block: Value) -> CommitteePageDoc {
        let mut value = sample();
        value["blocks"][index] = block;
        doc(value)
    }

    #[test]
    fn documents_round_trip_in_the_wire_shape() {
        let value = sample();
        let parsed = doc(value.clone());
        assert_eq!(
            parsed.blocks[2].content,
            BlockContent::WorkSessions(WorkSessionsProps {})
        );
        let back = serde_json::to_value(&parsed).unwrap();
        assert_eq!(back["blocks"][0]["type"], "cover");
        assert_eq!(
            back["blocks"][2],
            json!({ "id": "sessions", "type": "work_sessions", "width": 4, "props": {} })
        );
        assert_eq!(back["blocks"][1]["props"]["heading"], "What we do");
        assert_eq!(back["settings"]["meetingRoom"], "Tepper 2700");
        assert!(
            serde_json::from_value::<CommitteePageDoc>(json!({
                "blocks": [{ "id": "x", "type": "marquee", "width": 12, "props": {} }],
                "settings": {}
            }))
            .is_err()
        );
    }

    #[test]
    fn cleaning_trims_sanitizes_and_normalizes() {
        let mut value = sample();
        value["blocks"][1]["props"]["heading"] = json!("  What we do  ");
        value["blocks"][1]["props"]["body"] =
            json!("<p onclick=\"x()\">Hi<script>alert(1)</script></p>");
        let clean = clean_draft(doc(value)).unwrap();
        let BlockContent::Text(text) = &clean.blocks[1].content else {
            panic!()
        };
        assert_eq!(text.heading, "What we do");
        assert_eq!(text.body, "<p>Hi</p>");
        let BlockContent::Teams(teams) = &clean.blocks[4].content else {
            panic!()
        };
        assert_eq!(teams.teams[0].channel, "tech-maps");
        assert_eq!(clean.settings.slack_channel.as_deref(), Some("tech"));
        let BlockContent::Cta(cta) = &clean.blocks[5].content else {
            panic!()
        };
        assert_eq!(cta.secondary.as_ref().unwrap().url, "/sponsors/");
    }

    #[test]
    fn empty_rich_text_becomes_empty() {
        let mut value = sample();
        value["blocks"][1]["props"]["body"] = json!("<p><br></p><p> </p>");
        let clean = clean_draft(doc(value)).unwrap();
        let BlockContent::Text(text) = &clean.blocks[1].content else {
            panic!()
        };
        assert_eq!(text.body, "");
    }

    #[test]
    fn the_cover_comes_first_and_only_once() {
        let mut value = sample();
        let blocks = value["blocks"].as_array_mut().unwrap();
        blocks.swap(0, 1);
        assert!(fields(clean_draft(doc(value))).contains(&"blocks".to_owned()));

        let mut value = sample();
        let cover = value["blocks"][0].clone();
        let mut second = cover;
        second["id"] = json!("cover-2");
        value["blocks"].as_array_mut().unwrap().push(second);
        assert!(fields(clean_draft(doc(value))).contains(&"blocks.6".to_owned()));
    }

    #[test]
    fn pages_hold_at_most_twelve_blocks() {
        let mut value = sample();
        for n in 0..7 {
            value["blocks"].as_array_mut().unwrap().push(json!({
                "id": format!("quote-{n}"), "type": "quote", "width": 4, "props": { "text": "Hi", "attribution": "Me" }
            }));
        }
        assert_eq!(value["blocks"].as_array().unwrap().len(), 13);
        assert_eq!(fields(clean_draft(doc(value))), vec!["blocks".to_owned()]);
    }

    #[test]
    fn block_ids_are_unique_and_safe() {
        let mut value = sample();
        value["blocks"][2]["id"] = json!("what");
        assert_eq!(
            fields(clean_draft(doc(value))),
            vec!["blocks.2.id".to_owned()]
        );
        let mut value = sample();
        value["blocks"][2]["id"] = json!("<script>");
        assert_eq!(
            fields(clean_draft(doc(value))),
            vec!["blocks.2.id".to_owned()]
        );
    }

    #[test]
    fn widths_follow_the_block_type() {
        let mut value = sample();
        value["blocks"][2]["width"] = json!(12);
        value["blocks"][3]["width"] = json!(4);
        value["blocks"][1]["width"] = json!(5);
        let result = clean_draft(doc(value));
        assert_eq!(
            fields(result),
            vec![
                "blocks.1.width".to_owned(),
                "blocks.2.width".to_owned(),
                "blocks.3.width".to_owned()
            ]
        );
        assert_eq!(
            width_message(&[6, 8, 12]),
            "Make this block half, two thirds or full width."
        );
        assert_eq!(width_message(&[12]), "This block is always full width.");
    }

    #[test]
    fn stats_hold_two_to_four_numbers() {
        let one = json!({ "id": "stats", "type": "stats", "width": 12, "props": { "items": [{ "value": "1", "label": "x" }] } });
        assert_eq!(
            fields(clean_draft(with_block(3, one))),
            vec!["blocks.3.items".to_owned()]
        );
        let five: Vec<Value> = (0..5)
            .map(|n| json!({ "value": n.to_string(), "label": "x" }))
            .collect();
        let five =
            json!({ "id": "stats", "type": "stats", "width": 12, "props": { "items": five } });
        assert_eq!(
            fields(clean_draft(with_block(3, five))),
            vec!["blocks.3.items".to_owned()]
        );
        let long = json!({ "id": "stats", "type": "stats", "width": 12,
            "props": { "items": [{ "value": "1234567890123", "label": "x" }, { "value": "1", "label": "y" }] } });
        assert_eq!(
            fields(clean_draft(with_block(3, long))),
            vec!["blocks.3.items.0.value".to_owned()]
        );
    }

    #[test]
    fn galleries_fill_up_in_drafts_and_need_three_to_publish() {
        let photos = |n: usize| -> Vec<Value> {
            (0..n)
                .map(|_| image("/seed/photos/c01.jpg", "A photo"))
                .collect()
        };
        let gallery = |n: usize| {
            json!({ "id": "gallery", "type": "gallery", "width": 12,
            "props": { "images": photos(n), "layout": "thirds", "caption": "" } })
        };
        let draft = clean_draft(with_block(3, gallery(1))).unwrap();
        assert_eq!(
            fields(check_for_publishing(&draft)),
            vec!["blocks.3.images".to_owned()]
        );
        let full = clean_draft(with_block(3, gallery(3))).unwrap();
        assert!(check_for_publishing(&full).is_ok());
        assert_eq!(
            fields(clean_draft(with_block(3, gallery(7)))),
            vec!["blocks.3.images".to_owned()]
        );
    }

    #[test]
    fn teams_are_limited_and_channels_checked() {
        let many: Vec<Value> = (0..21)
            .map(|n| json!({ "name": format!("Team {n}"), "about": "", "channel": "" }))
            .collect();
        let block = json!({ "id": "teams", "type": "teams", "width": 12, "props": { "heading": "Teams", "intro": "", "teams": many } });
        assert_eq!(
            fields(clean_draft(with_block(4, block))),
            vec!["blocks.4.teams".to_owned()]
        );
        let bad = json!({ "id": "teams", "type": "teams", "width": 12,
            "props": { "heading": "Teams", "intro": "", "teams": [{ "name": "A", "about": "", "channel": "not a channel!" }] } });
        assert_eq!(
            fields(clean_draft(with_block(4, bad))),
            vec!["blocks.4.teams.0.channel".to_owned()]
        );
    }

    #[test]
    fn links_images_and_videos_are_checked() {
        let mut value = sample();
        value["blocks"][5]["props"]["primary"]["url"] = json!("javascript:alert(1)");
        value["blocks"][5]["props"]["secondary"] =
            json!({ "label": "", "url": "https://example.com" });
        value["settings"]["mapUrl"] = json!("http://maps.example.com");
        value["blocks"][0]["props"]["image"]["src"] = json!("https://evil.example/x.jpg");
        assert_eq!(
            fields(clean_draft(doc(value))),
            vec![
                "blocks.0.image".to_owned(),
                "blocks.5.primary.url".to_owned(),
                "blocks.5.secondary.label".to_owned(),
                "settings.mapUrl".to_owned()
            ]
        );
        let video = |url: &str| json!({ "id": "video", "type": "video", "width": 6, "props": { "url": url, "title": "Recap", "poster": null } });
        assert_eq!(
            fields(clean_draft(with_block(
                2,
                video("https://example.com/v.mp4")
            ))),
            vec!["blocks.2.url".to_owned()]
        );
        let empty = clean_draft(with_block(2, video("  "))).unwrap();
        assert_eq!(
            fields(check_for_publishing(&empty)),
            vec!["blocks.2.url".to_owned()]
        );
        assert!(clean_draft(with_block(2, video("https://youtu.be/dQw4w9WgXcQ"))).is_ok());
        for channel in [
            "https://www.youtube.com/@scottylabs",
            "https://www.youtube.com/playlist?list=PL1234567",
            "https://vimeo.com/scottylabs",
        ] {
            assert_eq!(
                fields(clean_draft(with_block(2, video(channel)))),
                vec!["blocks.2.url".to_owned()],
                "{channel} plays no single video"
            );
        }
    }

    #[test]
    fn video_links_are_read_like_the_page_reads_them() {
        for (link, id) in [
            ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            (
                "https://m.youtube.com/watch?v=dQw4w9WgXcQ&t=30",
                "dQw4w9WgXcQ",
            ),
            ("https://youtu.be/dQw4w9WgXcQ?si=abc", "dQw4w9WgXcQ"),
            ("https://www.youtube.com/embed/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://youtube.com/shorts/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://www.youtube.com/live/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://vimeo.com/76979871", "76979871"),
            ("https://vimeo.com/channels/staffpicks/76979871", "76979871"),
            ("https://player.vimeo.com/video/76979871", "76979871"),
        ] {
            assert_eq!(video_id(link).as_deref(), Some(id), "{link}");
        }
        for link in [
            "",
            "http://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/@scottylabs",
            "https://www.youtube.com/watch",
            "https://www.youtube.com/watch?v=abc",
            "https://www.youtube.com/watch?v=bad%20id%20here",
            "https://youtu.be/",
            "https://www.youtube.com/embed/",
            "https://vimeo.com/scottylabs",
            "https://player.vimeo.com/76979871",
            "https://example.com/watch?v=dQw4w9WgXcQ",
        ] {
            assert_eq!(video_id(link), None, "{link}");
        }
    }

    #[test]
    fn empty_buttons_are_dropped() {
        let mut value = sample();
        value["blocks"][5]["props"]["secondary"] = json!({ "label": " ", "url": "" });
        let clean = clean_draft(doc(value)).unwrap();
        let BlockContent::Cta(cta) = &clean.blocks[5].content else {
            panic!()
        };
        assert!(cta.secondary.is_none());
    }

    #[test]
    fn text_stays_within_limits() {
        let mut value = sample();
        value["blocks"][0]["props"]["tagline"] = json!("x".repeat(limits::TAGLINE + 1));
        value["blocks"][1]["props"]["body"] = json!(format!(
            "<p>{}</p>",
            "x".repeat(richtext::MAX_TEXT_CHARS + 1)
        ));
        value["settings"]["meetingTime"] = json!("two\nlines");
        assert_eq!(
            fields(clean_draft(doc(value))),
            vec![
                "blocks.0.tagline".to_owned(),
                "blocks.1.body".to_owned(),
                "settings.meetingTime".to_owned()
            ]
        );
    }

    #[test]
    fn publishing_needs_a_cover_photo_and_alt_text() {
        let mut value = sample();
        value["blocks"][0]["props"]["image"] = Value::Null;
        value["blocks"][2] = json!({ "id": "photo", "type": "image", "width": 4,
            "props": { "image": image("/media/0190e0a4-7c1b-7d2e-9f00-000000000001", " "), "caption": "" } });
        let draft = clean_draft(doc(value)).unwrap();
        assert_eq!(
            fields(check_for_publishing(&draft)),
            vec!["blocks.0.image".to_owned(), "blocks.2.image.alt".to_owned()]
        );
        assert!(check_for_publishing(&clean_draft(doc(sample())).unwrap()).is_ok());
    }

    #[test]
    fn media_ids_are_collected_once() {
        let id = "0190e0a4-7c1b-7d2e-9f00-000000000001";
        let mut value = sample();
        value["blocks"][0]["props"]["image"]["src"] = json!(format!("/media/{id}"));
        value["blocks"][2] = json!({ "id": "g", "type": "gallery", "width": 12,
            "props": { "images": [image(&format!("/media/{id}"), "a"), image("/seed/photos/c01.jpg", "b")], "layout": "mosaic", "caption": "" } });
        assert_eq!(doc(value).media_ids(), vec![Uuid::parse_str(id).unwrap()]);
    }

    #[test]
    fn changes_are_described_in_plain_sentences() {
        let published = doc(sample());
        let mut value = sample();
        let blocks = value["blocks"].as_array_mut().unwrap();
        let stats = blocks.remove(3);
        blocks.insert(1, stats);
        blocks[2]["width"] = json!(12);
        blocks[4]["props"]["intro"] = json!("Fifteen teams, each with its own channel.");
        blocks.insert(4, json!({ "id": "new-image", "type": "image", "width": 8, "props": { "image": null, "caption": "" } }));
        blocks.retain(|block| block["id"] != "cta");
        value["settings"]["meetingRoom"] = json!("Tepper 2701");
        let draft = doc(value);
        assert_eq!(
            describe_changes(Some(&published), &draft),
            vec![
                "Moved Stats above What we do",
                "Made What we do full width",
                "Added an image block",
                "Edited Teams",
                "Removed Call to action",
                "Changed the page settings",
            ]
        );
    }

    #[test]
    fn unchanged_and_unpublished_pages() {
        let page = doc(sample());
        assert!(describe_changes(Some(&page), &page).is_empty());
        assert_eq!(describe_changes(None, &page).len(), 1);
        let mut value = sample();
        let blocks = value["blocks"].as_array_mut().unwrap();
        let last = blocks.remove(1);
        blocks.push(last);
        value["blocks"][0]["props"]["tagline"] = json!("New tagline");
        assert_eq!(
            describe_changes(Some(&page), &doc(value)),
            vec!["Edited the cover", "Moved What we do below Call to action"]
        );
    }

    #[test]
    fn articles_read_naturally() {
        assert_eq!(with_article("Image"), "an image");
        assert_eq!(with_article("Call to action"), "a call to action");
        assert_eq!(with_article("Work sessions"), "a work sessions");
    }
}
