#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactTag(pub String);

pub struct Artifact {
    pub tag: ArtifactTag,
    pub consumers: Vec<String>,  // checksum of Rules that consume it
    pub creator: Option<String>, // checksum of the Tule that produces it
}

impl Artifact {
    pub(crate) fn new(tag: ArtifactTag) -> Self {
        Self {
            tag,
            creator: None,
            consumers: Vec::new(),
        }
    }

    pub(crate) fn creator_rule(&self) -> Option<String> {
        self.creator.clone()
    }

    pub(crate) fn consumer_rules(&self) -> Vec<String> {
        self.consumers.clone()
    }
}
