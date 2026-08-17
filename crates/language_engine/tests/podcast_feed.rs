use core_engine::TranscriptFormat;
use language_engine::{parse_podcast_feed, PodcastFeedIssue, PodcastFeedIssueDisposition};

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"
     xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
     xmlns:p="https://podcastindex.org/namespace/1.0">
  <channel>
    <title>Synthetic English</title>
    <language>en</language>
    <description>A synthetic feed.</description>
    <itunes:image href="/feed.png" />
    <item>
      <guid isPermaLink="false">episode-1</guid>
      <title>A Synthetic Episode</title>
      <pubDate>Mon, 17 Aug 2026 08:00:00 +0000</pubDate>
      <enclosure url="/audio/episode-1.mp3" type="audio/mpeg; charset=binary" />
      <itunes:duration>01:02:03</itunes:duration>
      <p:transcript url="/transcripts/episode-1.vtt" type="Text/VTT; charset=utf-8"
                    language="en" rel="captions" />
      <p:transcript url="/transcripts/episode-1.json" type="application/json" />
    </item>
  </channel>
</rss>"#;

const ATOM: &str = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
      xmlns:podcast="https://podcastindex.org/namespace/1.0"
      xml:lang="en">
  <title>Atom English</title>
  <subtitle>A synthetic Atom feed.</subtitle>
  <logo>/atom-cover.png</logo>
  <entry xml:lang="en-GB">
    <id>tag:example.test,2026:episode-atom</id>
    <title>An Atom Episode</title>
    <published>2026-08-17T08:00:00+00:00</published>
    <link rel="enclosure" href="/audio/atom.ogg" type="audio/ogg" />
    <itunes:duration>62</itunes:duration>
    <podcast:transcript url="/transcripts/atom.srt" type="APPLICATION/SRT"
                        language="en-GB" rel="captions" />
  </entry>
</feed>"#;

#[test]
fn rss_feed_maps_portable_metadata_and_supported_transcripts() {
    let report = parse_podcast_feed(
        "https://MEDIA.example.test:443/podcasts/feed.xml#top",
        RSS.as_bytes(),
    )
    .expect("synthetic RSS must parse");

    assert_eq!(
        report.feed.source_url,
        "https://media.example.test/podcasts/feed.xml"
    );
    assert_eq!(report.feed.title, "Synthetic English");
    assert_eq!(report.feed.language.as_deref(), Some("en"));
    assert_eq!(
        report.feed.description.as_deref(),
        Some("A synthetic feed.")
    );
    assert_eq!(
        report.feed.artwork_url.as_deref(),
        Some("https://media.example.test/feed.png")
    );
    assert!(report.issues.is_empty());
    assert_eq!(report.episodes.len(), 1);

    let episode = &report.episodes[0];
    assert_eq!(episode.title, "A Synthetic Episode");
    assert_eq!(episode.publisher_guid.as_deref(), Some("episode-1"));
    assert_eq!(episode.identity.feed_url, report.feed.source_url);
    assert_eq!(episode.identity.publisher_guid_aliases, ["episode-1"]);
    assert_eq!(
        episode.identity.enclosure_url_aliases,
        ["https://media.example.test/audio/episode-1.mp3"]
    );
    assert_eq!(episode.enclosure_mime_type, "audio/mpeg");
    assert_eq!(episode.duration_ms, Some(3_723_000));
    assert_eq!(episode.transcript_resources.len(), 2);
    assert_eq!(
        episode.transcript_resources[0].format,
        Some(TranscriptFormat::WebVtt)
    );
    assert_eq!(episode.transcript_resources[0].mime_type, "text/vtt");
    assert_eq!(episode.transcript_resources[1].format, None);
}

#[test]
fn invalid_entries_are_reported_without_discarding_playable_episodes() {
    let rss = RSS.replace(
        "</channel>",
        "<item><title>Missing Audio</title></item></channel>",
    );

    let report = parse_podcast_feed("https://media.example.test/feed.xml", rss.as_bytes())
        .expect("an invalid item must not invalidate the feed");

    assert_eq!(report.episodes.len(), 1);
    assert_eq!(
        report.issues,
        vec![PodcastFeedIssue::MissingAudioEnclosure { entry_index: 1 }]
    );
    assert_eq!(
        report.issues[0].disposition(),
        PodcastFeedIssueDisposition::EpisodeRejected
    );
    assert_eq!(
        report.issues[0].code(),
        "feed_issue_missing_audio_enclosure"
    );
}

#[test]
fn atom_feed_maps_default_namespace_metadata_and_enclosure_links() {
    let report = parse_podcast_feed("https://media.example.test/atom.xml", ATOM.as_bytes())
        .expect("synthetic Atom must parse");

    assert_eq!(report.feed.title, "Atom English");
    assert_eq!(report.feed.language.as_deref(), Some("en"));
    assert_eq!(
        report.feed.description.as_deref(),
        Some("A synthetic Atom feed.")
    );
    assert_eq!(
        report.feed.artwork_url.as_deref(),
        Some("https://media.example.test/atom-cover.png")
    );
    assert!(report.issues.is_empty());

    let episode = &report.episodes[0];
    assert_eq!(
        episode.publisher_guid.as_deref(),
        Some("tag:example.test,2026:episode-atom")
    );
    assert_eq!(episode.language.as_deref(), Some("en-GB"));
    assert_eq!(
        episode.enclosure_url,
        "https://media.example.test/audio/atom.ogg"
    );
    assert_eq!(episode.enclosure_mime_type, "audio/ogg");
    assert_eq!(episode.duration_ms, Some(62_000));
    assert_eq!(
        episode.transcript_resources[0].format,
        Some(TranscriptFormat::Srt)
    );
}

#[test]
fn fatal_feed_errors_have_stable_codes() {
    let cases: Vec<(&str, &str, Vec<u8>, &str)> = vec![
        (
            "invalid source URL",
            "not a URL",
            RSS.as_bytes().to_vec(),
            "feed_invalid_source_url",
        ),
        (
            "unsupported source scheme",
            "ftp://media.example.test/feed.xml",
            RSS.as_bytes().to_vec(),
            "feed_unsupported_source_scheme",
        ),
        (
            "invalid UTF-8",
            "https://media.example.test/feed.xml",
            vec![0xff, 0xfe, b'<', b'r'],
            "feed_invalid_encoding",
        ),
        (
            "unsupported XML encoding",
            "https://media.example.test/feed.xml",
            br#"<?xml version="1.0" encoding="ISO-8859-1"?><rss/>"#.to_vec(),
            "feed_unsupported_encoding",
        ),
        (
            "forbidden DTD",
            "https://media.example.test/feed.xml",
            br#"<!DOCTYPE rss><rss version="2.0"><channel><title>x</title></channel></rss>"#
                .to_vec(),
            "feed_forbidden_doctype",
        ),
        (
            "malformed XML",
            "https://media.example.test/feed.xml",
            br#"<rss><channel></rss>"#.to_vec(),
            "feed_malformed_xml",
        ),
        (
            "unsupported root",
            "https://media.example.test/feed.xml",
            br#"<rdf><title>x</title></rdf>"#.to_vec(),
            "feed_unsupported_format",
        ),
        (
            "missing feed title",
            "https://media.example.test/feed.xml",
            br#"<rss version="2.0"><channel><description>x</description></channel></rss>"#.to_vec(),
            "feed_missing_title",
        ),
    ];

    for (case, source_url, xml, expected_code) in cases {
        let error = parse_podcast_feed(source_url, &xml).expect_err(case);
        assert_eq!(error.code(), expected_code, "{case}");
    }
}

#[test]
fn entry_local_defects_return_typed_issues_and_preserve_valid_entries() {
    let rss = r#"<rss version="2.0" xmlns:p="https://podcastindex.org/namespace/1.0">
      <channel><title>Issue Matrix</title>
        <item><title>Valid</title><guid>same</guid>
          <enclosure url="/valid.mp3" type="audio/mpeg" />
        </item>
        <item><title>Duplicate</title><guid>same</guid>
          <enclosure url="/duplicate.mp3" type="audio/mpeg" />
        </item>
        <item><enclosure url="/untitled.mp3" type="audio/mpeg" /></item>
        <item><title>Bad Optional Data</title><pubDate>not-a-date</pubDate>
          <enclosure url="/optional.mp3" type="audio/mpeg" />
          <p:transcript type="text/vtt" />
          <p:transcript url="/captions.vtt" />
        </item>
      </channel>
    </rss>"#;

    let report = parse_podcast_feed("https://media.example.test/feed.xml", rss.as_bytes())
        .expect("item-local defects must not fail the feed");
    let codes: Vec<&str> = report.issues.iter().map(PodcastFeedIssue::code).collect();

    assert_eq!(report.episodes.len(), 2);
    assert_eq!(
        codes,
        vec![
            "feed_issue_duplicate_episode_identity",
            "feed_issue_missing_episode_title",
            "feed_issue_invalid_publication_date",
            "feed_issue_missing_transcript_url",
            "feed_issue_missing_transcript_mime_type",
        ]
    );
    assert_eq!(
        report.issues[0].disposition(),
        PodcastFeedIssueDisposition::EpisodeRejected
    );
    assert_eq!(
        report.issues[2].disposition(),
        PodcastFeedIssueDisposition::MetadataIgnored
    );
    assert_eq!(
        report.issues[3].disposition(),
        PodcastFeedIssueDisposition::TranscriptRejected
    );
}
