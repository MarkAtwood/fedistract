//! Podcast RSS feed generation.
//!
//! Pure logic module that generates podcast-compatible RSS 2.0 XML with
//! iTunes namespace extensions. No external dependencies needed -- just
//! string formatting.

/// Top-level podcast feed metadata.
pub struct PodcastFeed {
    pub title: String,
    pub description: String,
    pub link: String,
    pub author: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub itunes_category: Option<String>,
    pub cover_url: Option<String>,
    pub explicit: bool,
    pub episodes: Vec<PodcastEpisodeRss>,
}

/// A single podcast episode for RSS output.
pub struct PodcastEpisodeRss {
    pub title: String,
    pub description: String,
    pub enclosure_url: String,
    pub enclosure_length: u64,
    pub enclosure_type: String,
    pub duration_secs: Option<u32>,
    pub episode_number: Option<u32>,
    pub season_number: Option<u32>,
    pub published_at: String, // RFC 2822
    pub guid: String,
    pub explicit: bool,
}

/// Escape XML special characters in text content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Format a duration in seconds as HH:MM:SS.
fn format_duration(secs: u32) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Generate a complete podcast RSS 2.0 XML feed.
pub fn generate_rss(feed: &PodcastFeed) -> String {
    let mut xml = String::with_capacity(4096);

    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(
        r#"<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:content="http://purl.org/rss/1.0/modules/content/">"#,
    );
    xml.push_str("\n<channel>\n");

    xml.push_str(&format!("  <title>{}</title>\n", xml_escape(&feed.title)));
    xml.push_str(&format!(
        "  <description>{}</description>\n",
        xml_escape(&feed.description)
    ));
    xml.push_str(&format!("  <link>{}</link>\n", xml_escape(&feed.link)));
    xml.push_str(&format!(
        "  <itunes:author>{}</itunes:author>\n",
        xml_escape(&feed.author)
    ));

    if let Some(ref lang) = feed.language {
        xml.push_str(&format!("  <language>{}</language>\n", xml_escape(lang)));
    }

    if let Some(ref cat) = feed.category {
        xml.push_str(&format!(
            "  <category>{}</category>\n",
            xml_escape(cat)
        ));
    }

    if let Some(ref icat) = feed.itunes_category {
        xml.push_str(&format!(
            "  <itunes:category text=\"{}\" />\n",
            xml_escape(icat)
        ));
    }

    if let Some(ref url) = feed.cover_url {
        xml.push_str(&format!(
            "  <itunes:image href=\"{}\" />\n",
            xml_escape(url)
        ));
        xml.push_str(&format!(
            "  <image>\n    <url>{}</url>\n    <title>{}</title>\n    <link>{}</link>\n  </image>\n",
            xml_escape(url),
            xml_escape(&feed.title),
            xml_escape(&feed.link)
        ));
    }

    xml.push_str(&format!(
        "  <itunes:explicit>{}</itunes:explicit>\n",
        if feed.explicit { "true" } else { "false" }
    ));

    for ep in &feed.episodes {
        xml.push_str("  <item>\n");
        xml.push_str(&format!(
            "    <title>{}</title>\n",
            xml_escape(&ep.title)
        ));
        xml.push_str(&format!(
            "    <description>{}</description>\n",
            xml_escape(&ep.description)
        ));
        xml.push_str(&format!(
            "    <enclosure url=\"{}\" length=\"{}\" type=\"{}\" />\n",
            xml_escape(&ep.enclosure_url),
            ep.enclosure_length,
            xml_escape(&ep.enclosure_type)
        ));
        xml.push_str(&format!(
            "    <guid isPermaLink=\"false\">{}</guid>\n",
            xml_escape(&ep.guid)
        ));
        xml.push_str(&format!(
            "    <pubDate>{}</pubDate>\n",
            xml_escape(&ep.published_at)
        ));

        if let Some(dur) = ep.duration_secs {
            xml.push_str(&format!(
                "    <itunes:duration>{}</itunes:duration>\n",
                format_duration(dur)
            ));
        }

        if let Some(num) = ep.episode_number {
            xml.push_str(&format!(
                "    <itunes:episode>{num}</itunes:episode>\n"
            ));
        }

        if let Some(sn) = ep.season_number {
            xml.push_str(&format!(
                "    <itunes:season>{sn}</itunes:season>\n"
            ));
        }

        xml.push_str(&format!(
            "    <itunes:explicit>{}</itunes:explicit>\n",
            if ep.explicit { "true" } else { "false" }
        ));

        xml.push_str("  </item>\n");
    }

    xml.push_str("</channel>\n</rss>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_feed() -> PodcastFeed {
        PodcastFeed {
            title: "Test Podcast".into(),
            description: "A test podcast about testing".into(),
            link: "https://example.com/podcast".into(),
            author: "Test Author".into(),
            language: Some("en".into()),
            category: Some("Technology".into()),
            itunes_category: Some("Technology".into()),
            cover_url: Some("https://example.com/cover.jpg".into()),
            explicit: false,
            episodes: vec![
                PodcastEpisodeRss {
                    title: "Episode 1".into(),
                    description: "First episode".into(),
                    enclosure_url: "https://example.com/ep1.mp3".into(),
                    enclosure_length: 50_000_000,
                    enclosure_type: "audio/mpeg".into(),
                    duration_secs: Some(3600),
                    episode_number: Some(1),
                    season_number: Some(1),
                    published_at: "Mon, 01 Jan 2024 00:00:00 +0000".into(),
                    guid: "https://example.com/ep/1".into(),
                    explicit: false,
                },
                PodcastEpisodeRss {
                    title: "Episode 2".into(),
                    description: "Second episode".into(),
                    enclosure_url: "https://example.com/ep2.mp3".into(),
                    enclosure_length: 45_000_000,
                    enclosure_type: "audio/mpeg".into(),
                    duration_secs: Some(2700),
                    episode_number: Some(2),
                    season_number: Some(1),
                    published_at: "Mon, 08 Jan 2024 00:00:00 +0000".into(),
                    guid: "https://example.com/ep/2".into(),
                    explicit: true,
                },
            ],
        }
    }

    #[test]
    fn generates_valid_xml_header() {
        let xml = generate_rss(&sample_feed());
        assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(xml.contains(r#"<rss version="2.0""#));
        assert!(xml.contains("xmlns:itunes="));
    }

    #[test]
    fn contains_channel_metadata() {
        let xml = generate_rss(&sample_feed());
        assert!(xml.contains("<title>Test Podcast</title>"));
        assert!(xml.contains("<description>A test podcast about testing</description>"));
        assert!(xml.contains("<link>https://example.com/podcast</link>"));
        assert!(xml.contains("<itunes:author>Test Author</itunes:author>"));
        assert!(xml.contains("<language>en</language>"));
        assert!(xml.contains("<category>Technology</category>"));
        assert!(xml.contains(r#"<itunes:category text="Technology" />"#));
        assert!(xml.contains(r#"<itunes:image href="https://example.com/cover.jpg" />"#));
        assert!(xml.contains("<itunes:explicit>false</itunes:explicit>"));
    }

    #[test]
    fn contains_episodes() {
        let xml = generate_rss(&sample_feed());
        assert!(xml.contains("<title>Episode 1</title>"));
        assert!(xml.contains("<title>Episode 2</title>"));
        assert!(xml.contains(r#"<enclosure url="https://example.com/ep1.mp3" length="50000000" type="audio/mpeg" />"#));
        assert!(xml.contains(r#"<guid isPermaLink="false">https://example.com/ep/1</guid>"#));
        assert!(xml.contains("<pubDate>Mon, 01 Jan 2024 00:00:00 +0000</pubDate>"));
        assert!(xml.contains("<itunes:duration>01:00:00</itunes:duration>"));
        assert!(xml.contains("<itunes:episode>1</itunes:episode>"));
        assert!(xml.contains("<itunes:season>1</itunes:season>"));
    }

    #[test]
    fn episode_explicit_flag() {
        let xml = generate_rss(&sample_feed());
        // Episode 2 is explicit
        assert!(xml.contains("<itunes:explicit>true</itunes:explicit>"));
    }

    #[test]
    fn xml_escape_works() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(xml_escape("it's"), "it&apos;s");
        assert_eq!(xml_escape("plain text"), "plain text");
    }

    #[test]
    fn format_duration_works() {
        assert_eq!(format_duration(0), "00:00:00");
        assert_eq!(format_duration(59), "00:00:59");
        assert_eq!(format_duration(60), "00:01:00");
        assert_eq!(format_duration(3600), "01:00:00");
        assert_eq!(format_duration(3661), "01:01:01");
        assert_eq!(format_duration(7200), "02:00:00");
    }

    #[test]
    fn escapes_special_chars_in_feed() {
        let feed = PodcastFeed {
            title: "Tom & Jerry's <Show>".into(),
            description: "A \"great\" podcast".into(),
            link: "https://example.com".into(),
            author: "Author & Co".into(),
            language: None,
            category: None,
            itunes_category: None,
            cover_url: None,
            explicit: false,
            episodes: vec![],
        };
        let xml = generate_rss(&feed);
        assert!(xml.contains("<title>Tom &amp; Jerry&apos;s &lt;Show&gt;</title>"));
        assert!(xml.contains("<description>A &quot;great&quot; podcast</description>"));
    }

    #[test]
    fn empty_episodes() {
        let feed = PodcastFeed {
            title: "Empty Pod".into(),
            description: "No episodes".into(),
            link: "https://example.com".into(),
            author: "Nobody".into(),
            language: None,
            category: None,
            itunes_category: None,
            cover_url: None,
            explicit: false,
            episodes: vec![],
        };
        let xml = generate_rss(&feed);
        assert!(xml.contains("<title>Empty Pod</title>"));
        assert!(!xml.contains("<item>"));
        assert!(xml.ends_with("</channel>\n</rss>\n"));
    }

    #[test]
    fn optional_fields_omitted() {
        let feed = PodcastFeed {
            title: "Minimal".into(),
            description: "".into(),
            link: "https://example.com".into(),
            author: "Author".into(),
            language: None,
            category: None,
            itunes_category: None,
            cover_url: None,
            explicit: false,
            episodes: vec![PodcastEpisodeRss {
                title: "Ep".into(),
                description: "".into(),
                enclosure_url: "https://example.com/ep.mp3".into(),
                enclosure_length: 1000,
                enclosure_type: "audio/mpeg".into(),
                duration_secs: None,
                episode_number: None,
                season_number: None,
                published_at: "Mon, 01 Jan 2024 00:00:00 +0000".into(),
                guid: "guid-1".into(),
                explicit: false,
            }],
        };
        let xml = generate_rss(&feed);
        assert!(!xml.contains("<language>"));
        assert!(!xml.contains("<category>"));
        assert!(!xml.contains("<itunes:category"));
        assert!(!xml.contains("<itunes:image"));
        assert!(!xml.contains("<itunes:duration>"));
        assert!(!xml.contains("<itunes:episode>"));
        assert!(!xml.contains("<itunes:season>"));
    }

    #[test]
    fn feed_closes_properly() {
        let xml = generate_rss(&sample_feed());
        assert!(xml.ends_with("</channel>\n</rss>\n"));
    }
}
