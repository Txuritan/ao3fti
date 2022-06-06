pub struct Story {
    pub id: usize,
    pub name: String,
    pub summary: String,
    pub authors: Vec<Entity>,
    pub origins: Vec<Entity>,
    pub warnings: Vec<Entity>,
    pub pairings: Vec<Entity>,
    pub characters: Vec<Entity>,
    pub generals: Vec<Entity>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Rating {
    #[serde(rename = "explicit")]
    Explicit,
    #[serde(rename = "mature")]
    Mature,
    #[serde(rename = "teen")]
    Teen,
    #[serde(rename = "general")]
    General,
    #[serde(rename = "not-rated")]
    NotRated,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub name: String,
}
