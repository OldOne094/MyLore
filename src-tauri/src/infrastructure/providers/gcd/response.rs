//! GCD API response models (MISSION-109omics).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub results: Vec<Series>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Series {
    #[serde(default, rename = "api_url")]
    pub api_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub year_began: Option<i64>,
    #[serde(default)]
    pub year_ended: Option<i64>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default, rename = "issue_descriptors")]
    pub issue_descriptors: Option<Vec<String>>,
    #[serde(default, rename = "active_issues")]
    pub active_issues: Option<Vec<String>>,
    #[serde(skip)]
    #[allow(dead_code)]
    pub _extra: Option<()>,
}
