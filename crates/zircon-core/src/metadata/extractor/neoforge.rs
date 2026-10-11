use crate::model::{ModLoaderType, ModMetadata};
use super::MetadataError;
use super::forge::parse_toml_metadata;

pub fn parse_neoforge_metadata(content: &str) -> Result<ModMetadata, MetadataError> {
    parse_toml_metadata(content, ModLoaderType::NeoForge)
}
