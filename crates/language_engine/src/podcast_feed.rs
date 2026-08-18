use std::collections::HashMap;
use std::str;

use chrono::{DateTime, Utc};
use core_engine::{
    EpisodeIdentity, PodcastEpisode, PodcastFeed, TranscriptFormat, TranscriptResource,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const PODCAST_NAMESPACE: &str = "https://podcastindex.org/namespace/1.0";
const ATOM_NAMESPACE: &str = "http://www.w3.org/2005/Atom";
const ITUNES_NAMESPACE_HTTP: &str = "http://www.itunes.com/dtds/podcast-1.0.dtd";
const ITUNES_NAMESPACE_HTTPS: &str = "https://www.itunes.com/dtds/podcast-1.0.dtd";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodcastFeedParseReport {
    pub feed: PodcastFeed,
    pub episodes: Vec<PodcastEpisode>,
    pub issues: Vec<PodcastFeedIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodcastFeedIssueDisposition {
    EpisodeRejected,
    MetadataIgnored,
    TranscriptRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PodcastFeedIssue {
    InvalidFeedArtworkUrl,
    MissingEpisodeTitle {
        entry_index: usize,
    },
    MissingAudioEnclosure {
        entry_index: usize,
    },
    MissingEnclosureMimeType {
        entry_index: usize,
    },
    InvalidEnclosureUrl {
        entry_index: usize,
    },
    InvalidPublicationDate {
        entry_index: usize,
    },
    InvalidDuration {
        entry_index: usize,
    },
    InvalidEpisodeArtworkUrl {
        entry_index: usize,
    },
    MissingTranscriptUrl {
        entry_index: usize,
        transcript_index: usize,
    },
    MissingTranscriptMimeType {
        entry_index: usize,
        transcript_index: usize,
    },
    InvalidTranscriptUrl {
        entry_index: usize,
        transcript_index: usize,
    },
    DuplicateEpisodeIdentity {
        entry_index: usize,
        first_entry_index: usize,
    },
}

impl PodcastFeedIssue {
    pub const fn disposition(&self) -> PodcastFeedIssueDisposition {
        match self {
            Self::MissingEpisodeTitle { .. }
            | Self::MissingAudioEnclosure { .. }
            | Self::MissingEnclosureMimeType { .. }
            | Self::InvalidEnclosureUrl { .. }
            | Self::DuplicateEpisodeIdentity { .. } => PodcastFeedIssueDisposition::EpisodeRejected,
            Self::MissingTranscriptUrl { .. }
            | Self::MissingTranscriptMimeType { .. }
            | Self::InvalidTranscriptUrl { .. } => PodcastFeedIssueDisposition::TranscriptRejected,
            Self::InvalidFeedArtworkUrl
            | Self::InvalidPublicationDate { .. }
            | Self::InvalidDuration { .. }
            | Self::InvalidEpisodeArtworkUrl { .. } => PodcastFeedIssueDisposition::MetadataIgnored,
        }
    }

    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidFeedArtworkUrl => "feed_issue_invalid_feed_artwork_url",
            Self::MissingEpisodeTitle { .. } => "feed_issue_missing_episode_title",
            Self::MissingAudioEnclosure { .. } => "feed_issue_missing_audio_enclosure",
            Self::MissingEnclosureMimeType { .. } => "feed_issue_missing_enclosure_mime_type",
            Self::InvalidEnclosureUrl { .. } => "feed_issue_invalid_enclosure_url",
            Self::InvalidPublicationDate { .. } => "feed_issue_invalid_publication_date",
            Self::InvalidDuration { .. } => "feed_issue_invalid_duration",
            Self::InvalidEpisodeArtworkUrl { .. } => "feed_issue_invalid_episode_artwork_url",
            Self::MissingTranscriptUrl { .. } => "feed_issue_missing_transcript_url",
            Self::MissingTranscriptMimeType { .. } => "feed_issue_missing_transcript_mime_type",
            Self::InvalidTranscriptUrl { .. } => "feed_issue_invalid_transcript_url",
            Self::DuplicateEpisodeIdentity { .. } => "feed_issue_duplicate_episode_identity",
        }
    }
}

#[derive(Debug, Error)]
pub enum PodcastFeedParseError {
    #[error("feed source URL is invalid")]
    InvalidSourceUrl,
    #[error("feed source URL must use HTTP or HTTPS")]
    UnsupportedSourceScheme,
    #[error("feed source encoding is invalid at byte {byte_offset}")]
    InvalidEncoding { byte_offset: usize },
    #[error("feed declares unsupported XML encoding {encoding}")]
    UnsupportedEncoding { encoding: String },
    #[error("feed XML document types are not supported at byte {byte_offset}")]
    ForbiddenDoctype { byte_offset: u64 },
    #[error("feed XML is malformed at byte {byte_offset}: {message}")]
    MalformedXml { byte_offset: u64, message: String },
    #[error("feed format {root} is unsupported")]
    UnsupportedFeedFormat { root: String },
    #[error("feed title is missing")]
    MissingFeedTitle,
}

impl PodcastFeedParseError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSourceUrl => "feed_invalid_source_url",
            Self::UnsupportedSourceScheme => "feed_unsupported_source_scheme",
            Self::InvalidEncoding { .. } => "feed_invalid_encoding",
            Self::UnsupportedEncoding { .. } => "feed_unsupported_encoding",
            Self::ForbiddenDoctype { .. } => "feed_forbidden_doctype",
            Self::MalformedXml { .. } => "feed_malformed_xml",
            Self::UnsupportedFeedFormat { .. } => "feed_unsupported_format",
            Self::MissingFeedTitle => "feed_missing_title",
        }
    }
}

pub fn parse_podcast_feed(
    source_url: &str,
    xml: &[u8],
) -> Result<PodcastFeedParseReport, PodcastFeedParseError> {
    let source_url = canonical_source_url(source_url)?;
    let root = parse_xml(xml)?;
    if root.namespace.is_none() && root.name == "rss" {
        parse_rss(&source_url, &root)
    } else if root.namespace.as_deref() == Some(ATOM_NAMESPACE) && root.name == "feed" {
        parse_atom(&source_url, &root)
    } else {
        Err(PodcastFeedParseError::UnsupportedFeedFormat { root: root.name })
    }
}

fn parse_atom(
    source_url: &Url,
    root: &XmlNode,
) -> Result<PodcastFeedParseReport, PodcastFeedParseError> {
    let title = root
        .child_text(Some(ATOM_NAMESPACE), "title")
        .ok_or(PodcastFeedParseError::MissingFeedTitle)?;
    let language = root.attr("lang").and_then(nonempty);
    let description = root.child_text(Some(ATOM_NAMESPACE), "subtitle");
    let mut issues = Vec::new();
    let artwork_url = root
        .child_text(Some(ATOM_NAMESPACE), "logo")
        .or_else(|| root.child_text(Some(ATOM_NAMESPACE), "icon"))
        .map(|value| canonical_resource_url(source_url, &value))
        .transpose()
        .unwrap_or_else(|_| {
            issues.push(PodcastFeedIssue::InvalidFeedArtworkUrl);
            None
        })
        .map(|url| url.to_string());
    let feed = PodcastFeed {
        source_url: source_url.to_string(),
        title,
        language,
        description,
        artwork_url,
    };

    let mut episodes = Vec::new();
    let mut identities = HashMap::new();
    for (entry_index, entry) in root.children(Some(ATOM_NAMESPACE), "entry").enumerate() {
        if let Some(episode) = parse_atom_entry(source_url, &feed, entry, entry_index, &mut issues)
        {
            if let Some(first_entry_index) = identities.get(&episode.identity.internal_id) {
                issues.push(PodcastFeedIssue::DuplicateEpisodeIdentity {
                    entry_index,
                    first_entry_index: *first_entry_index,
                });
            } else {
                identities.insert(episode.identity.internal_id.clone(), entry_index);
                episodes.push(episode);
            }
        }
    }

    Ok(PodcastFeedParseReport {
        feed,
        episodes,
        issues,
    })
}

fn parse_atom_entry(
    source_url: &Url,
    feed: &PodcastFeed,
    entry: &XmlNode,
    entry_index: usize,
    issues: &mut Vec<PodcastFeedIssue>,
) -> Option<PodcastEpisode> {
    let Some(title) = entry.child_text(Some(ATOM_NAMESPACE), "title") else {
        issues.push(PodcastFeedIssue::MissingEpisodeTitle { entry_index });
        return None;
    };
    let enclosure_nodes: Vec<&XmlNode> = entry
        .children(Some(ATOM_NAMESPACE), "link")
        .filter(|node| {
            node.attr("rel")
                .is_some_and(|rel| rel.eq_ignore_ascii_case("enclosure"))
        })
        .collect();
    if enclosure_nodes.is_empty() {
        issues.push(PodcastFeedIssue::MissingAudioEnclosure { entry_index });
        return None;
    }
    let mut selected_enclosure = None;
    let mut saw_missing_type = false;
    let mut saw_invalid_url = false;
    for enclosure in enclosure_nodes {
        let Some(mime_type) = enclosure.attr("type").and_then(normalize_mime) else {
            saw_missing_type = true;
            continue;
        };
        if !mime_type.starts_with("audio/") {
            continue;
        }
        let Some(url) = enclosure.attr("href") else {
            saw_invalid_url = true;
            continue;
        };
        match canonical_resource_url(source_url, url) {
            Ok(url) => {
                selected_enclosure = Some((url, mime_type));
                break;
            }
            Err(()) => saw_invalid_url = true,
        }
    }
    let Some((enclosure_url, enclosure_mime_type)) = selected_enclosure else {
        issues.push(if saw_missing_type {
            PodcastFeedIssue::MissingEnclosureMimeType { entry_index }
        } else if saw_invalid_url {
            PodcastFeedIssue::InvalidEnclosureUrl { entry_index }
        } else {
            PodcastFeedIssue::MissingAudioEnclosure { entry_index }
        });
        return None;
    };

    let publisher_guid = entry.child_text(Some(ATOM_NAMESPACE), "id");
    let published_at = entry
        .child_text(Some(ATOM_NAMESPACE), "published")
        .or_else(|| entry.child_text(Some(ATOM_NAMESPACE), "updated"))
        .and_then(|value| match DateTime::parse_from_rfc3339(&value) {
            Ok(value) => Some(value.with_timezone(&Utc)),
            Err(_) => {
                issues.push(PodcastFeedIssue::InvalidPublicationDate { entry_index });
                None
            }
        });
    let duration_ms = entry
        .child_text_in(is_itunes_namespace, "duration")
        .and_then(|value| match parse_duration_ms(&value) {
            Some(value) => Some(value),
            None => {
                issues.push(PodcastFeedIssue::InvalidDuration { entry_index });
                None
            }
        });
    let artwork_url = entry
        .child_in(is_itunes_namespace, "image")
        .and_then(|node| node.attr("href"))
        .map(|value| canonical_resource_url(source_url, value))
        .transpose()
        .unwrap_or_else(|_| {
            issues.push(PodcastFeedIssue::InvalidEpisodeArtworkUrl { entry_index });
            None
        })
        .map(|url| url.to_string())
        .or_else(|| feed.artwork_url.clone());
    let transcript_resources = parse_transcript_resources(source_url, entry, entry_index, issues);
    let enclosure_url = enclosure_url.to_string();
    let identity = episode_identity(source_url, publisher_guid.as_deref(), &enclosure_url);

    Some(PodcastEpisode {
        identity,
        publisher_guid,
        title,
        published_at,
        language: entry
            .attr("lang")
            .and_then(nonempty)
            .or_else(|| feed.language.clone()),
        enclosure_url,
        enclosure_mime_type,
        duration_ms,
        artwork_url,
        transcript_resources,
    })
}

fn parse_rss(
    source_url: &Url,
    root: &XmlNode,
) -> Result<PodcastFeedParseReport, PodcastFeedParseError> {
    let channel = root
        .child(None, "channel")
        .ok_or(PodcastFeedParseError::MissingFeedTitle)?;
    let title = channel
        .child_text(None, "title")
        .ok_or(PodcastFeedParseError::MissingFeedTitle)?;
    let language = channel.child_text(None, "language");
    let description = channel.child_text(None, "description");
    let mut issues = Vec::new();
    let artwork_url = rss_artwork(channel)
        .map(|value| canonical_resource_url(source_url, &value))
        .transpose()
        .unwrap_or_else(|_| {
            issues.push(PodcastFeedIssue::InvalidFeedArtworkUrl);
            None
        })
        .map(|url| url.to_string());
    let feed = PodcastFeed {
        source_url: source_url.to_string(),
        title,
        language,
        description,
        artwork_url,
    };

    let mut episodes = Vec::new();
    let mut identities = HashMap::new();
    for (entry_index, item) in channel.children(None, "item").enumerate() {
        if let Some(episode) = parse_rss_item(source_url, &feed, item, entry_index, &mut issues) {
            if let Some(first_entry_index) = identities.get(&episode.identity.internal_id) {
                issues.push(PodcastFeedIssue::DuplicateEpisodeIdentity {
                    entry_index,
                    first_entry_index: *first_entry_index,
                });
            } else {
                identities.insert(episode.identity.internal_id.clone(), entry_index);
                episodes.push(episode);
            }
        }
    }

    Ok(PodcastFeedParseReport {
        feed,
        episodes,
        issues,
    })
}

fn parse_rss_item(
    source_url: &Url,
    feed: &PodcastFeed,
    item: &XmlNode,
    entry_index: usize,
    issues: &mut Vec<PodcastFeedIssue>,
) -> Option<PodcastEpisode> {
    let Some(title) = item.child_text(None, "title") else {
        issues.push(PodcastFeedIssue::MissingEpisodeTitle { entry_index });
        return None;
    };

    let enclosure_nodes: Vec<&XmlNode> = item.children(None, "enclosure").collect();
    if enclosure_nodes.is_empty() {
        issues.push(PodcastFeedIssue::MissingAudioEnclosure { entry_index });
        return None;
    }
    let mut selected_enclosure = None;
    let mut saw_audio_without_type = false;
    let mut saw_invalid_url = false;
    for enclosure in enclosure_nodes {
        let Some(mime_type) = enclosure.attr("type").and_then(normalize_mime) else {
            saw_audio_without_type = true;
            continue;
        };
        if !mime_type.starts_with("audio/") {
            continue;
        }
        let Some(url) = enclosure.attr("url") else {
            saw_invalid_url = true;
            continue;
        };
        match canonical_resource_url(source_url, url) {
            Ok(url) => {
                selected_enclosure = Some((url, mime_type));
                break;
            }
            Err(()) => saw_invalid_url = true,
        }
    }
    let Some((enclosure_url, enclosure_mime_type)) = selected_enclosure else {
        issues.push(if saw_audio_without_type {
            PodcastFeedIssue::MissingEnclosureMimeType { entry_index }
        } else if saw_invalid_url {
            PodcastFeedIssue::InvalidEnclosureUrl { entry_index }
        } else {
            PodcastFeedIssue::MissingAudioEnclosure { entry_index }
        });
        return None;
    };

    let publisher_guid = item.child_text(None, "guid");
    let published_at = item.child_text(None, "pubDate").and_then(|value| {
        match DateTime::parse_from_rfc2822(&value) {
            Ok(value) => Some(value.with_timezone(&Utc)),
            Err(_) => {
                issues.push(PodcastFeedIssue::InvalidPublicationDate { entry_index });
                None
            }
        }
    });
    let duration_ms = item
        .child_text_in(is_itunes_namespace, "duration")
        .and_then(|value| match parse_duration_ms(&value) {
            Some(value) => Some(value),
            None => {
                issues.push(PodcastFeedIssue::InvalidDuration { entry_index });
                None
            }
        });
    let artwork_url = item
        .child_in(is_itunes_namespace, "image")
        .and_then(|node| node.attr("href"))
        .map(|value| canonical_resource_url(source_url, value))
        .transpose()
        .unwrap_or_else(|_| {
            issues.push(PodcastFeedIssue::InvalidEpisodeArtworkUrl { entry_index });
            None
        })
        .map(|url| url.to_string())
        .or_else(|| feed.artwork_url.clone());
    let transcript_resources = parse_transcript_resources(source_url, item, entry_index, issues);
    let enclosure_url = enclosure_url.to_string();
    let identity = episode_identity(source_url, publisher_guid.as_deref(), &enclosure_url);

    Some(PodcastEpisode {
        identity,
        publisher_guid,
        title,
        published_at,
        language: item
            .child_text(None, "language")
            .or_else(|| feed.language.clone()),
        enclosure_url,
        enclosure_mime_type,
        duration_ms,
        artwork_url,
        transcript_resources,
    })
}

pub(crate) fn episode_identity(
    source_url: &Url,
    publisher_guid: Option<&str>,
    enclosure_url: &str,
) -> EpisodeIdentity {
    let identity_key = match publisher_guid {
        Some(guid) => format!("ensub:podcast-episode:{}:guid:{guid}", source_url.as_str()),
        None => format!(
            "ensub:podcast-episode:{}:enclosure:{enclosure_url}",
            source_url.as_str()
        ),
    };
    EpisodeIdentity {
        internal_id: Uuid::new_v5(&Uuid::NAMESPACE_URL, identity_key.as_bytes()).to_string(),
        feed_url: source_url.to_string(),
        publisher_guid_aliases: publisher_guid.into_iter().map(str::to_string).collect(),
        enclosure_url_aliases: vec![enclosure_url.to_string()],
    }
}

fn parse_transcript_resources(
    source_url: &Url,
    item: &XmlNode,
    entry_index: usize,
    issues: &mut Vec<PodcastFeedIssue>,
) -> Vec<TranscriptResource> {
    item.children(Some(PODCAST_NAMESPACE), "transcript")
        .enumerate()
        .filter_map(|(transcript_index, node)| {
            let Some(url) = node.attr("url") else {
                issues.push(PodcastFeedIssue::MissingTranscriptUrl {
                    entry_index,
                    transcript_index,
                });
                return None;
            };
            let Some(mime_type) = node.attr("type").and_then(normalize_mime) else {
                issues.push(PodcastFeedIssue::MissingTranscriptMimeType {
                    entry_index,
                    transcript_index,
                });
                return None;
            };
            let url = match canonical_resource_url(source_url, url) {
                Ok(url) => url.to_string(),
                Err(()) => {
                    issues.push(PodcastFeedIssue::InvalidTranscriptUrl {
                        entry_index,
                        transcript_index,
                    });
                    return None;
                }
            };
            let format = match mime_type.as_str() {
                "text/vtt" => Some(TranscriptFormat::WebVtt),
                "application/x-subrip" | "application/srt" | "text/srt" => {
                    Some(TranscriptFormat::Srt)
                }
                _ => None,
            };
            Some(TranscriptResource {
                url,
                mime_type,
                format,
                language: node.attr("language").and_then(nonempty),
                relation: node.attr("rel").and_then(nonempty),
            })
        })
        .collect()
}

fn canonical_source_url(value: &str) -> Result<Url, PodcastFeedParseError> {
    let url = Url::parse(value.trim()).map_err(|_| PodcastFeedParseError::InvalidSourceUrl)?;
    canonical_http_url(url).map_err(|error| match error {
        UrlProblem::Scheme => PodcastFeedParseError::UnsupportedSourceScheme,
        UrlProblem::Invalid => PodcastFeedParseError::InvalidSourceUrl,
    })
}

fn canonical_resource_url(base: &Url, value: &str) -> Result<Url, ()> {
    let url = base.join(value.trim()).map_err(|_| ())?;
    canonical_http_url(url).map_err(|_| ())
}

enum UrlProblem {
    Scheme,
    Invalid,
}

fn canonical_http_url(mut url: Url) -> Result<Url, UrlProblem> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(UrlProblem::Scheme);
    }
    if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err(UrlProblem::Invalid);
    }
    url.set_fragment(None);
    Ok(url)
}

fn normalize_mime(value: &str) -> Option<String> {
    nonempty(value.split(';').next().unwrap_or_default()).map(|value| value.to_lowercase())
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!value.is_empty()).then_some(value)
}

fn parse_duration_ms(value: &str) -> Option<u64> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    let seconds = match parts.as_slice() {
        [seconds] => seconds.parse::<u64>().ok()?,
        [minutes, seconds] => {
            let minutes = minutes.parse::<u64>().ok()?;
            let seconds = seconds.parse::<u64>().ok()?;
            if seconds >= 60 {
                return None;
            }
            minutes.checked_mul(60)?.checked_add(seconds)?
        }
        [hours, minutes, seconds] => {
            let hours = hours.parse::<u64>().ok()?;
            let minutes = minutes.parse::<u64>().ok()?;
            let seconds = seconds.parse::<u64>().ok()?;
            if minutes >= 60 || seconds >= 60 {
                return None;
            }
            hours
                .checked_mul(3_600)?
                .checked_add(minutes.checked_mul(60)?)?
                .checked_add(seconds)?
        }
        _ => return None,
    };
    seconds.checked_mul(1_000)
}

fn rss_artwork(channel: &XmlNode) -> Option<String> {
    channel
        .child_in(is_itunes_namespace, "image")
        .and_then(|node| node.attr("href"))
        .and_then(nonempty)
        .or_else(|| {
            channel
                .child(None, "image")
                .and_then(|image| image.child_text(None, "url"))
        })
}

fn is_itunes_namespace(namespace: Option<&str>) -> bool {
    matches!(
        namespace,
        Some(ITUNES_NAMESPACE_HTTP | ITUNES_NAMESPACE_HTTPS)
    )
}

#[derive(Debug)]
struct XmlNode {
    namespace: Option<String>,
    name: String,
    attributes: Vec<(String, String)>,
    text: String,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_str()))
    }

    fn child(&self, namespace: Option<&str>, name: &str) -> Option<&Self> {
        self.children
            .iter()
            .find(|child| child.namespace.as_deref() == namespace && child.name == name)
    }

    fn child_in(&self, namespace: impl Fn(Option<&str>) -> bool, name: &str) -> Option<&Self> {
        self.children
            .iter()
            .find(|child| namespace(child.namespace.as_deref()) && child.name == name)
    }

    fn children<'a>(
        &'a self,
        namespace: Option<&'a str>,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Self> {
        self.children
            .iter()
            .filter(move |child| child.namespace.as_deref() == namespace && child.name == name)
    }

    fn child_text(&self, namespace: Option<&str>, name: &str) -> Option<String> {
        self.child(namespace, name)
            .and_then(|node| nonempty(&node.text))
    }

    fn child_text_in(
        &self,
        namespace: impl Fn(Option<&str>) -> bool,
        name: &str,
    ) -> Option<String> {
        self.child_in(namespace, name)
            .and_then(|node| nonempty(&node.text))
    }
}

fn parse_xml(xml: &[u8]) -> Result<XmlNode, PodcastFeedParseError> {
    let xml = xml.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(xml);
    str::from_utf8(xml).map_err(|error| PodcastFeedParseError::InvalidEncoding {
        byte_offset: error.valid_up_to(),
    })?;
    let mut reader = NsReader::from_reader(xml);
    let mut stack = Vec::new();
    let mut root = None;

    loop {
        let position = reader.buffer_position();
        let decoder = reader.decoder();
        let (namespace, event) =
            reader
                .read_resolved_event()
                .map_err(|error| PodcastFeedParseError::MalformedXml {
                    byte_offset: position,
                    message: error.to_string(),
                })?;
        match event {
            Event::Decl(declaration) => {
                if let Some(encoding) = declaration.encoding() {
                    let encoding =
                        encoding.map_err(|error| PodcastFeedParseError::MalformedXml {
                            byte_offset: position,
                            message: error.to_string(),
                        })?;
                    let encoding = str::from_utf8(&encoding).map_err(|_| {
                        PodcastFeedParseError::InvalidEncoding {
                            byte_offset: position as usize,
                        }
                    })?;
                    if !encoding.eq_ignore_ascii_case("utf-8")
                        && !encoding.eq_ignore_ascii_case("us-ascii")
                    {
                        return Err(PodcastFeedParseError::UnsupportedEncoding {
                            encoding: encoding.to_string(),
                        });
                    }
                }
            }
            Event::DocType(_) => {
                return Err(PodcastFeedParseError::ForbiddenDoctype {
                    byte_offset: position,
                });
            }
            Event::Start(start) => {
                let node = xml_node(namespace, &start, decoder, position)?;
                stack.push(node);
            }
            Event::Empty(start) => {
                let node = xml_node(namespace, &start, decoder, position)?;
                append_node(&mut stack, &mut root, node, position)?;
            }
            Event::Text(text) => {
                let decoded =
                    text.decode()
                        .map_err(|error| PodcastFeedParseError::MalformedXml {
                            byte_offset: position,
                            message: error.to_string(),
                        })?;
                let decoded = quick_xml::escape::unescape(&decoded).map_err(|error| {
                    PodcastFeedParseError::MalformedXml {
                        byte_offset: position,
                        message: error.to_string(),
                    }
                })?;
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&decoded);
                } else if !decoded.trim().is_empty() {
                    return Err(PodcastFeedParseError::MalformedXml {
                        byte_offset: position,
                        message: "text outside the root element".to_string(),
                    });
                }
            }
            Event::CData(text) => {
                let decoded =
                    text.decode()
                        .map_err(|error| PodcastFeedParseError::MalformedXml {
                            byte_offset: position,
                            message: error.to_string(),
                        })?;
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&decoded);
                }
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| PodcastFeedParseError::MalformedXml {
                        byte_offset: position,
                        message: "closing element without an open element".to_string(),
                    })?;
                append_node(&mut stack, &mut root, node, position)?;
            }
            Event::Eof => break,
            Event::Comment(_) | Event::PI(_) | Event::GeneralRef(_) => {}
        }
    }
    if !stack.is_empty() {
        return Err(PodcastFeedParseError::MalformedXml {
            byte_offset: reader.buffer_position(),
            message: "unexpected end of XML".to_string(),
        });
    }
    root.ok_or_else(|| PodcastFeedParseError::UnsupportedFeedFormat {
        root: "empty_document".to_string(),
    })
}

fn xml_node(
    namespace: ResolveResult<'_>,
    start: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    position: u64,
) -> Result<XmlNode, PodcastFeedParseError> {
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(
            str::from_utf8(namespace.as_ref())
                .map_err(|_| PodcastFeedParseError::InvalidEncoding {
                    byte_offset: position as usize,
                })?
                .to_string(),
        ),
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(prefix) => {
            return Err(PodcastFeedParseError::MalformedXml {
                byte_offset: position,
                message: format!(
                    "unbound namespace prefix {}",
                    String::from_utf8_lossy(&prefix)
                ),
            });
        }
    };
    let name = str::from_utf8(start.local_name().as_ref())
        .map_err(|_| PodcastFeedParseError::InvalidEncoding {
            byte_offset: position as usize,
        })?
        .to_string();
    let mut attributes = Vec::new();
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|error| PodcastFeedParseError::MalformedXml {
            byte_offset: position,
            message: error.to_string(),
        })?;
        let name = str::from_utf8(attribute.key.local_name().as_ref())
            .map_err(|_| PodcastFeedParseError::InvalidEncoding {
                byte_offset: position as usize,
            })?
            .to_string();
        let value = attribute
            .decode_and_unescape_value(decoder)
            .map_err(|error| PodcastFeedParseError::MalformedXml {
                byte_offset: position,
                message: error.to_string(),
            })?
            .into_owned();
        attributes.push((name, value));
    }
    Ok(XmlNode {
        namespace,
        name,
        attributes,
        text: String::new(),
        children: Vec::new(),
    })
}

fn append_node(
    stack: &mut [XmlNode],
    root: &mut Option<XmlNode>,
    node: XmlNode,
    position: u64,
) -> Result<(), PodcastFeedParseError> {
    if let Some(parent) = stack.last_mut() {
        parent.text.push_str(&node.text);
        parent.children.push(node);
        Ok(())
    } else if root.is_none() {
        *root = Some(node);
        Ok(())
    } else {
        Err(PodcastFeedParseError::MalformedXml {
            byte_offset: position,
            message: "multiple root elements".to_string(),
        })
    }
}
