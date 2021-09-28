use serde::Deserialize;

#[derive(Deserialize)]
pub struct Search {
    pub q: String,
    #[serde(alias = "type")]
    pub search_type: Option<String>,
}
