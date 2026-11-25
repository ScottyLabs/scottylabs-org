//! Announcement rows, the shapes the portal sends, and the rules every announcement follows.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::{AppResult, Problems};
use crate::models::{TickerSource, check_https_url, check_line, text_enum};

/// Messages are one line of at most this many characters.
pub const MAX_MESSAGE: usize = 90;
/// An announcement leaves the ticker at most this many days after it goes live.
pub const MAX_DAYS: i64 = 30;
/// Approving fails when the chosen expiry is this close, so nothing goes live for a few seconds.
const APPROVAL_MARGIN_MINUTES: i64 = 5;

text_enum!(
    /// What an announcement is about. Chooses its icon.
    AnnouncementKind {
        Event => "event",
        ProjectUpdate => "project_update",
        Milestone => "milestone",
        ClubNews => "club_news",
    }
);

text_enum!(
    /// `expired` is never stored: it is a live announcement whose `expires_at` has passed.
    AnnouncementState {
        PendingApproval => "pending_approval",
        Live => "live",
        SentBack => "sent_back",
        Ended => "ended",
        Expired => "expired",
    }
);

#[derive(Clone, Debug, FromRow)]
pub struct Row {
    pub id: Uuid,
    pub kind: AnnouncementKind,
    pub committee: TickerSource,
    pub message: String,
    pub link_url: Option<String>,
    pub state: AnnouncementState,
    pub starts_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub position: i32,
    pub created_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Row {
    pub const COLUMNS: &'static str = "id, kind, committee, message, link_url, state, starts_at, \
         expires_at, ended_at, position, created_by, approved_by, created_at, updated_at";

    /// On the ticker right now.
    pub fn is_live(&self, now: DateTime<Utc>) -> bool {
        self.state == AnnouncementState::Live && self.expires_at > now
    }

    /// Pending approval or sent back to the lead.
    pub fn is_waiting(&self) -> bool {
        matches!(
            self.state,
            AnnouncementState::PendingApproval | AnnouncementState::SentBack
        )
    }

    /// Live or waiting, so it can still be edited. Expired and ended ones are posted again instead.
    pub fn is_open(&self, now: DateTime<Utc>) -> bool {
        self.is_live(now) || self.is_waiting()
    }

    pub fn shown_state(&self, now: DateTime<Utc>) -> AnnouncementState {
        if self.state == AnnouncementState::Live && self.expires_at <= now {
            AnnouncementState::Expired
        } else {
            self.state
        }
    }

    /// The fields a person chooses, as submitted for approval.
    pub fn input(&self) -> AnnouncementInput {
        AnnouncementInput {
            kind: self.kind,
            committee: self.committee,
            message: self.message.clone(),
            link_url: self.link_url.clone(),
            expires_at: self.expires_at,
        }
    }
}

/// Mirrors `AnnouncementInput` in types.ts. Also the snapshot stored with an approval.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementInput {
    pub kind: AnnouncementKind,
    pub committee: TickerSource,
    pub message: String,
    #[serde(default)]
    pub link_url: Option<String>,
    pub expires_at: DateTime<Utc>,
}

/// `PATCH /api/announcements/:id`: any subset of [`AnnouncementInput`]. `linkUrl: null` removes the link.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementPatch {
    kind: Option<AnnouncementKind>,
    committee: Option<TickerSource>,
    message: Option<String>,
    #[serde(default, deserialize_with = "present")]
    link_url: Option<Option<String>>,
    expires_at: Option<DateTime<Utc>>,
}

/// Tells a field sent as `null` (`Some(None)`) from a field left out (`None`).
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl AnnouncementPatch {
    pub fn apply_to(self, current: AnnouncementInput) -> AnnouncementInput {
        AnnouncementInput {
            kind: self.kind.unwrap_or(current.kind),
            committee: self.committee.unwrap_or(current.committee),
            message: self.message.unwrap_or(current.message),
            link_url: self.link_url.unwrap_or(current.link_url),
            expires_at: self.expires_at.unwrap_or(current.expires_at),
        }
    }
}

/// An [`AnnouncementInput`] that passed [`validate`]: trimmed message, normalized link.
#[derive(Clone, Debug)]
pub struct Valid {
    pub kind: AnnouncementKind,
    pub committee: TickerSource,
    pub message: String,
    pub link_url: Option<String>,
    pub expires_at: DateTime<Utc>,
}

/// Checks every field and reports all problems at once, keyed by field.
pub fn validate(input: &AnnouncementInput, now: DateTime<Utc>) -> AppResult<Valid> {
    let mut problems = Problems::new();
    let message = check_line(&input.message, 1, MAX_MESSAGE, "the message")
        .map_err(|message| problems.add("message", message))
        .unwrap_or_default();
    let link_url = match input.link_url.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(raw) => check_https_url(raw)
            .map_err(|message| problems.add("linkUrl", message))
            .ok(),
    };
    if let Err(message) = check_expiry(input.expires_at, now) {
        problems.add("expiresAt", message);
    }
    problems.finish()?;
    Ok(Valid {
        kind: input.kind,
        committee: input.committee,
        message,
        link_url,
        expires_at: input.expires_at,
    })
}

/// An expiry must be in the future and at most 30 days away, since everything goes live now.
pub fn check_expiry(expires_at: DateTime<Utc>, now: DateTime<Utc>) -> Result<(), String> {
    if expires_at <= now {
        return Err("Pick a time in the future for it to leave the ticker.".into());
    }
    if expires_at > now + TimeDelta::days(MAX_DAYS) {
        return Err(format!(
            "Pick a time within {MAX_DAYS} days. Announcements stay on the ticker for {MAX_DAYS} days at most."
        ));
    }
    Ok(())
}

/// Whether a submitted expiry has passed, or is too close for the announcement to be worth showing.
pub fn approval_too_late(expires_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    expires_at <= now + TimeDelta::minutes(APPROVAL_MARGIN_MINUTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(message: &str, link: Option<&str>, expires_in: TimeDelta) -> AnnouncementInput {
        AnnouncementInput {
            kind: AnnouncementKind::Event,
            committee: TickerSource::Events,
            message: message.into(),
            link_url: link.map(Into::into),
            expires_at: Utc::now() + expires_in,
        }
    }

    #[test]
    fn messages_are_one_trimmed_line_of_at_most_90() {
        let now = Utc::now();
        let ok = validate(&input("  Hello there.  ", None, TimeDelta::days(1)), now).unwrap();
        assert_eq!(ok.message, "Hello there.");
        assert_eq!(ok.link_url, None);
        assert!(validate(&input(&"x".repeat(90), None, TimeDelta::days(1)), now).is_ok());

        for bad in ["", "   ", "two\nlines", &"x".repeat(91)] {
            let Err(crate::error::AppError::Validation { fields, .. }) =
                validate(&input(bad, None, TimeDelta::days(1)), now)
            else {
                panic!("{bad:?} should fail");
            };
            assert!(fields.contains_key("message"), "{bad:?}");
        }
        // Characters, not bytes.
        assert!(validate(&input(&"\u{e9}".repeat(90), None, TimeDelta::days(1)), now).is_ok());
    }

    #[test]
    fn links_are_optional_and_https() {
        let now = Utc::now();
        let blank = validate(&input("Hi", Some("  "), TimeDelta::days(1)), now).unwrap();
        assert_eq!(blank.link_url, None);
        let ok = validate(
            &input("Hi", Some(" https://tartanhacks.com "), TimeDelta::days(1)),
            now,
        )
        .unwrap();
        assert_eq!(ok.link_url.as_deref(), Some("https://tartanhacks.com/"));
        for bad in [
            "http://tartanhacks.com",
            "javascript:alert(1)",
            "tartanhacks.com",
        ] {
            let Err(crate::error::AppError::Validation { fields, .. }) =
                validate(&input("Hi", Some(bad), TimeDelta::days(1)), now)
            else {
                panic!("{bad} should fail");
            };
            assert!(fields.contains_key("linkUrl"));
        }
    }

    #[test]
    fn expiry_is_in_the_next_30_days() {
        let now = Utc::now();
        assert!(check_expiry(now + TimeDelta::minutes(1), now).is_ok());
        assert!(check_expiry(now + TimeDelta::days(30), now).is_ok());
        assert!(check_expiry(now, now).is_err());
        assert!(check_expiry(now - TimeDelta::hours(1), now).is_err());
        assert!(check_expiry(now + TimeDelta::days(30) + TimeDelta::seconds(1), now).is_err());
        assert!(approval_too_late(now + TimeDelta::minutes(4), now));
        assert!(!approval_too_late(now + TimeDelta::minutes(6), now));
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let Err(crate::error::AppError::Validation { fields, .. }) = validate(
            &input("", Some("http://x.org"), -TimeDelta::days(1)),
            Utc::now(),
        ) else {
            panic!("should fail");
        };
        assert_eq!(fields.len(), 3);
    }

    #[test]
    fn patches_keep_what_they_leave_out() {
        let current = input("Hi", Some("https://a.org/"), TimeDelta::days(2));
        let same = AnnouncementPatch::default().apply_to(current.clone());
        assert_eq!(same.link_url, current.link_url);
        let patch: AnnouncementPatch =
            serde_json::from_str(r#"{"message":"New","linkUrl":null}"#).unwrap();
        let changed = patch.apply_to(current.clone());
        assert_eq!(changed.message, "New");
        assert_eq!(changed.link_url, None);
        assert_eq!(changed.expires_at, current.expires_at);
    }

    #[test]
    fn expired_is_derived_from_the_expiry() {
        let now = Utc::now();
        let mut row = Row {
            id: Uuid::now_v7(),
            kind: AnnouncementKind::Milestone,
            committee: TickerSource::Club,
            message: "Hi".into(),
            link_url: None,
            state: AnnouncementState::Live,
            starts_at: Some(now),
            expires_at: now + TimeDelta::days(1),
            ended_at: None,
            position: 0,
            created_by: None,
            approved_by: None,
            created_at: now,
            updated_at: now,
        };
        assert!(row.is_live(now));
        assert_eq!(row.shown_state(now), AnnouncementState::Live);
        row.expires_at = now - TimeDelta::seconds(1);
        assert!(!row.is_live(now));
        assert!(!row.is_open(now));
        assert_eq!(row.shown_state(now), AnnouncementState::Expired);
        row.state = AnnouncementState::SentBack;
        assert!(row.is_open(now));
        assert_eq!(row.shown_state(now), AnnouncementState::SentBack);
    }
}
