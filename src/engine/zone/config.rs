/// Zone configuration that defines the properties of a zone
#[derive(Debug, Clone)]
pub struct ZoneConfig {
    /// How many tabs might be opened in the zone
    pub max_tabs: usize,
}