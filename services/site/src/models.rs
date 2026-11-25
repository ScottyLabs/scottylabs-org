//! Types and checks shared by every part of the API.

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

/// A string-backed enum stored as `text` and sent as its slug.
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $slug:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name { $($variant),+ }

        impl $name {
            /// Every variant, in declaration order. Only some of the enums built here list their variants.
            #[allow(dead_code)]
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn slug(self) -> &'static str {
                match self { $($name::$variant => $slug),+ }
            }

            pub fn from_slug(slug: &str) -> Option<Self> {
                match slug { $($slug => Some($name::$variant),)+ _ => None }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.slug())
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.slug())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = <String as ::serde::Deserialize>::deserialize(deserializer)?;
                $name::from_slug(&raw).ok_or_else(|| {
                    <D::Error as ::serde::de::Error>::custom(format!(
                        concat!("unknown ", stringify!($name), " {:?}"),
                        raw
                    ))
                })
            }
        }

        impl ::sqlx::Type<::sqlx::Postgres> for $name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <String as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &::sqlx::postgres::PgTypeInfo) -> bool {
                <String as ::sqlx::Type<::sqlx::Postgres>>::compatible(ty)
            }
        }

        impl<'r> ::sqlx::Decode<'r, ::sqlx::Postgres> for $name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, ::sqlx::error::BoxDynError> {
                let raw = <&str as ::sqlx::Decode<::sqlx::Postgres>>::decode(value)?;
                $name::from_slug(raw).ok_or_else(|| {
                    format!(concat!("unknown ", stringify!($name), " {:?}"), raw).into()
                })
            }
        }

        impl ::sqlx::Encode<'_, ::sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer,
            ) -> Result<::sqlx::encode::IsNull, ::sqlx::error::BoxDynError> {
                <&str as ::sqlx::Encode<::sqlx::Postgres>>::encode(self.slug(), buf)
            }
        }
    };
}

pub(crate) use text_enum;

text_enum!(
    /// The seven committees, in the site's order.
    Committee {
        Events => "events",
        Outreach => "outreach",
        Tech => "tech",
        Labrador => "labrador",
        Design => "design",
        Finance => "finance",
        Foundry => "foundry",
    }
);

impl Committee {
    pub fn name(self) -> &'static str {
        match self {
            Self::Events => "Events",
            Self::Outreach => "Outreach",
            Self::Tech => "Tech",
            Self::Labrador => "Labrador",
            Self::Design => "Design",
            Self::Finance => "Finance",
            Self::Foundry => "Foundry",
        }
    }
}

text_enum!(
    /// Who a ticker announcement is from.
    TickerSource {
        Club => "club",
        Events => "events",
        Outreach => "outreach",
        Tech => "tech",
        Labrador => "labrador",
        Design => "design",
        Finance => "finance",
        Foundry => "foundry",
    }
);

impl TickerSource {
    pub fn committee(self) -> Option<Committee> {
        Committee::from_slug(self.slug())
    }

    pub fn name(self) -> &'static str {
        self.committee().map_or("ScottyLabs", Committee::name)
    }
}

impl From<Committee> for TickerSource {
    fn from(committee: Committee) -> Self {
        // Every committee slug is also a ticker source slug.
        TickerSource::from_slug(committee.slug()).unwrap_or(TickerSource::Club)
    }
}

text_enum!(Role {
    Admin => "admin",
    Director => "director",
    Lead => "lead",
});

text_enum!(AuthMode {
    Oidc => "oidc",
    Dev => "dev",
});

text_enum!(RoleSource {
    KeycloakGroup => "keycloak_group",
    Governance => "governance",
    Portal => "portal",
});

/// A person as other people see them.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRef {
    pub id: Uuid,
    pub andrew_id: String,
    pub name: String,
    pub initials: String,
}

impl PersonRef {
    pub fn new(id: Uuid, andrew_id: String, name: String) -> Self {
        let initials = initials(&name, &andrew_id);
        let name = if name.trim().is_empty() {
            andrew_id.clone()
        } else {
            name
        };
        Self {
            id,
            andrew_id,
            name,
            initials,
        }
    }
}

/// Up to two capital letters from the first and last word of a name, or the Andrew ID.
pub fn initials(name: &str, andrew_id: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let letters: String = match words.as_slice() {
        [] => andrew_id.chars().take(2).collect(),
        [only] => only.chars().take(1).collect(),
        [first, .., last] => first.chars().take(1).chain(last.chars().take(1)).collect(),
    };
    letters.to_uppercase()
}

/// An image on a page. `src` is `/media/<uuid>` or `/seed/<path>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRef {
    pub src: String,
    #[serde(default)]
    pub alt: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

pub const MAX_ALT_LEN: usize = 300;

impl ImageRef {
    /// Checks the source and alt text. `require_alt` is for publishing.
    pub fn check(&self, require_alt: bool) -> Result<(), String> {
        if !is_local_image_src(&self.src) {
            return Err("Choose an image uploaded to the portal.".into());
        }
        if self.alt.chars().count() > MAX_ALT_LEN {
            return Err(format!(
                "Keep the image description under {MAX_ALT_LEN} characters."
            ));
        }
        if require_alt && self.alt.trim().is_empty() {
            return Err("Describe the image for people using screen readers.".into());
        }
        Ok(())
    }

    /// The uploaded media id, if this is an upload.
    pub fn media_id(&self) -> Option<Uuid> {
        self.src
            .strip_prefix("/media/")
            .and_then(|id| Uuid::parse_str(id).ok())
    }
}

fn is_local_image_src(src: &str) -> bool {
    if let Some(id) = src.strip_prefix("/media/") {
        return Uuid::parse_str(id).is_ok() && id.len() == 36;
    }
    if let Some(path) = src.strip_prefix("/seed/") {
        return !path.is_empty()
            && !path.contains("..")
            && path
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'));
    }
    false
}

pub const MAX_URL_LEN: usize = 2048;

/// Trims and checks an `https` URL. Returns the normalized string.
pub fn check_https_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.len() > MAX_URL_LEN {
        return Err("That link is too long.".into());
    }
    let url =
        Url::parse(trimmed).map_err(|_| "Enter a full link starting with https://".to_owned())?;
    if url.scheme() != "https" || url.host_str().is_none_or(str::is_empty) {
        return Err("Links must start with https://".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Links can't include a username or password.".into());
    }
    Ok(url.to_string())
}

/// Like [`check_https_url`] but also allows `mailto:` addresses and paths on this site such as `/sponsors/`, for buttons.
pub fn check_button_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('/') {
        return check_site_path(trimmed);
    }
    if let Some(address) = trimmed.strip_prefix("mailto:") {
        let valid = address.len() <= 254
            && address.split_once('@').is_some_and(|(user, domain)| {
                !user.is_empty() && domain.contains('.') && !address.contains(char::is_whitespace)
            });
        return if valid {
            Ok(trimmed.to_owned())
        } else {
            Err("Enter an email address after mailto:".into())
        };
    }
    check_https_url(trimmed)
}

/// A path on this site, such as `/sponsors/`. `//host` and backslashes are refused because browsers read them as links to other sites.
fn check_site_path(path: &str) -> Result<String, String> {
    const ERROR: &str =
        "Enter a path on this site that starts with a single /, such as /sponsors/.";
    let suspicious = path.starts_with("//")
        || path.contains('\\')
        || path.chars().any(|c| c.is_whitespace() || c.is_control());
    if suspicious || path.len() > MAX_URL_LEN {
        return Err(ERROR.into());
    }
    let base = Url::parse("https://scottylabs.invalid/").map_err(|_| ERROR.to_owned())?;
    let joined = base.join(path).map_err(|_| ERROR.to_owned())?;
    if joined.origin() != base.origin() {
        return Err(ERROR.into());
    }
    Ok(path.to_owned())
}

/// A YouTube or Vimeo link, normalized. Returns an error message otherwise.
pub fn check_video_url(raw: &str) -> Result<String, String> {
    let normalized = check_https_url(raw)?;
    let url = Url::parse(&normalized).map_err(|_| "Enter a YouTube or Vimeo link.".to_owned())?;
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.");
    if matches!(
        host,
        "youtube.com" | "m.youtube.com" | "youtu.be" | "vimeo.com" | "player.vimeo.com"
    ) {
        Ok(normalized)
    } else {
        Err("Use a YouTube or Vimeo link.".into())
    }
}

/// Trims and checks a single-line text field.
pub fn check_line(raw: &str, min: usize, max: usize, label: &str) -> Result<String, String> {
    let value = raw.trim().to_owned();
    if value.contains(['\n', '\r']) {
        return Err(format!("Keep {label} on one line."));
    }
    if value.chars().any(is_hidden_control) {
        return Err(format!("Remove the special characters from {label}."));
    }
    let count = value.chars().count();
    if count < min {
        return Err(if min == 1 {
            format!("Add {label}.")
        } else {
            format!("Write at least {min} characters for {label}.")
        });
    }
    if count > max {
        return Err(format!("Keep {label} to {max} characters or fewer."));
    }
    Ok(value)
}

/// Characters a one-line field refuses besides line breaks: control characters such as NUL and tab, the Unicode line and paragraph separators, and the bidirectional embeddings, overrides and isolates, which can make text read backwards or spoof what it says.
fn is_hidden_control(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committees_round_trip() {
        for committee in Committee::ALL {
            assert_eq!(Committee::from_slug(committee.slug()), Some(*committee));
            assert_eq!(TickerSource::from(*committee).committee(), Some(*committee));
        }
        assert_eq!(TickerSource::Club.committee(), None);
        assert_eq!(serde_json::to_string(&Committee::Tech).unwrap(), "\"tech\"");
        assert!(serde_json::from_str::<Committee>("\"marketing\"").is_err());
    }

    #[test]
    fn initials_cover_common_names() {
        assert_eq!(initials("Thomas Kanz", "tkanz"), "TK");
        assert_eq!(initials("Siddarth Radha Krishnan", "x"), "SK");
        assert_eq!(initials("Cher", "x"), "C");
        assert_eq!(initials("  ", "tkanz"), "TK");
    }

    #[test]
    fn image_sources_must_be_local() {
        let ok = ImageRef {
            src: "/media/0190e0a4-7c1b-7d2e-9f00-000000000001".into(),
            alt: "Students".into(),
            width: None,
            height: None,
        };
        assert!(ok.check(true).is_ok());
        assert!(ok.media_id().is_some());
        let seed = ImageRef {
            src: "/seed/committees/tech.jpg".into(),
            ..ok.clone()
        };
        assert!(seed.check(true).is_ok());
        for bad in [
            "https://evil.example/x.png",
            "/seed/../etc/passwd",
            "/media/not-a-uuid",
            "javascript:alert(1)",
        ] {
            let image = ImageRef {
                src: bad.into(),
                ..ok.clone()
            };
            assert!(image.check(false).is_err(), "{bad} should be rejected");
        }
        let no_alt = ImageRef {
            alt: " ".into(),
            ..ok
        };
        assert!(no_alt.check(false).is_ok());
        assert!(no_alt.check(true).is_err());
    }

    #[test]
    fn urls_must_be_https() {
        assert!(check_https_url("https://collegecart.org").is_ok());
        assert!(check_https_url("http://collegecart.org").is_err());
        assert!(check_https_url("javascript:alert(1)").is_err());
        assert!(check_https_url("https://user:pw@example.com").is_err());
        assert!(check_button_url("mailto:hello@scottylabs.org").is_ok());
        assert!(check_button_url("mailto:nope").is_err());
        assert_eq!(check_button_url(" /sponsors/ ").unwrap(), "/sponsors/");
        for bad in ["//evil.example", "/\\evil.example", "/a b", "/x\u{0}"] {
            assert!(check_button_url(bad).is_err(), "{bad:?} should be rejected");
        }
        assert_eq!(
            check_button_url("/\\evil").unwrap_err(),
            "Enter a path on this site that starts with a single /, such as /sponsors/."
        );
        assert!(check_video_url("https://www.youtube.com/watch?v=abc").is_ok());
        assert!(check_video_url("https://youtu.be/abc").is_ok());
        assert!(check_video_url("https://vimeo.com/123").is_ok());
        assert!(check_video_url("https://example.com/video.mp4").is_err());
    }

    #[test]
    fn lines_are_trimmed_and_bounded() {
        assert_eq!(check_line("  hi  ", 1, 10, "a name").unwrap(), "hi");
        assert!(check_line("", 1, 10, "a name").is_err());
        assert!(check_line("a\nb", 1, 10, "a name").is_err());
        for hidden in ["\0", "\t", "\u{7}", "\u{2028}", "\u{202E}", "\u{2067}"] {
            assert_eq!(
                check_line(&format!("a{hidden}b"), 1, 10, "the message"),
                Err("Remove the special characters from the message.".to_owned()),
                "{hidden:?}"
            );
        }
        // Letters from any script, emoji sequences and directional marks stay.
        assert!(
            check_line(
                "Caf\u{e9} \u{1F469}\u{200D}\u{1F4BB} \u{5E9}\u{200F}",
                1,
                20,
                "a name"
            )
            .is_ok()
        );
        assert!(check_line(&"x".repeat(11), 1, 10, "a name").is_err());
    }
}
