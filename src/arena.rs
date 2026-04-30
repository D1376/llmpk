use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::rsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slug {
    Text,
    Search,
    Vision,
    Document,
    Code,
    TextToImage,
    ImageEdit,
    TextToVideo,
    ImageToVideo,
    VideoEdit,
}

impl Slug {
    pub const ALL: [Slug; 10] = [
        Slug::Text,
        Slug::Search,
        Slug::Vision,
        Slug::Document,
        Slug::Code,
        Slug::TextToImage,
        Slug::ImageEdit,
        Slug::TextToVideo,
        Slug::ImageToVideo,
        Slug::VideoEdit,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Slug::Text => "text",
            Slug::Search => "search",
            Slug::Vision => "vision",
            Slug::Document => "document",
            Slug::Code => "code",
            Slug::TextToImage => "text-to-image",
            Slug::ImageEdit => "image-edit",
            Slug::TextToVideo => "text-to-video",
            Slug::ImageToVideo => "image-to-video",
            Slug::VideoEdit => "video-edit",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Slug::Text => "Text",
            Slug::Search => "Search",
            Slug::Vision => "Vision",
            Slug::Document => "Document",
            Slug::Code => "Code",
            Slug::TextToImage => "T2I",
            Slug::ImageEdit => "ImgEdit",
            Slug::TextToVideo => "T2V",
            Slug::ImageToVideo => "I2V",
            Slug::VideoEdit => "VidEdit",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Slug::TextToImage | Slug::ImageEdit => Kind::Image,
            Slug::TextToVideo | Slug::ImageToVideo | Slug::VideoEdit => Kind::Video,
            _ => Kind::Text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Image,
    Video,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub rank: Option<u32>,
    #[serde(default, rename = "modelDisplayName")]
    pub name: String,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub votes: Option<u64>,
    #[serde(default, rename = "modelOrganization")]
    pub organization: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default, rename = "inputPricePerMillion")]
    pub input_price: Option<f64>,
    #[serde(default, rename = "outputPricePerMillion")]
    pub output_price: Option<f64>,
    #[serde(default, rename = "contextLength")]
    pub context_length: Option<u64>,
    #[serde(default, rename = "pricePerImage")]
    pub price_per_image: Option<f64>,
    #[serde(default, rename = "pricePerSecond")]
    pub price_per_second: Option<f64>,
}

pub fn fetch(slug: Slug) -> Result<Vec<Entry>> {
    let url = format!("https://arena.ai/leaderboard/{}", slug.path());
    let html = rsc::fetch_html(&url)?;
    parse(&html)
}

pub fn parse(html: &str) -> Result<Vec<Entry>> {
    let stream = rsc::extract_stream(html)?;
    let arr = rsc::first_array_after(&stream, "entries")
        .ok_or_else(|| anyhow!("no entries array in arena.ai page"))?;
    let entries: Vec<Entry> = serde_json::from_str(arr)?;
    if entries.is_empty() {
        return Err(anyhow!("entries array empty"));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_fetch_when_enabled() {
        if std::env::var("LLMPK_LIVE").is_err() {
            return;
        }
        let entries = fetch(Slug::Text).expect("live arena fetch");
        assert!(entries.len() > 5);
    }

    #[test]
    fn parses_fixtures_when_provided() {
        let Ok(dir) = std::env::var("LLMPK_ARENA_FIXTURE_DIR") else {
            return;
        };
        for slug in Slug::ALL {
            let path = format!("{dir}/arena_{}.html", slug.path());
            let html = std::fs::read_to_string(&path).expect(&path);
            let entries = parse(&html).expect(slug.path());
            assert!(!entries.is_empty(), "{} empty", slug.path());
        }
    }
}
