use interpulse::api::{minecraft::VersionManifest, modded::CURRENT_LEGACY_FABRIC_FORMAT_VERSION};

use crate::utils::*;

// nightmare nightmare nightmare nightmare
const LEGACY_FABRIC_META_URL: &str = "https://meta.legacyfabric.net/v2";
const LEGACY_FABRIC_MAVEN_URL: &str = "https://repo.legacyfabric.net/repository/legacyfabric/";

pub async fn retrieve_data(
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	crate::utils::fabric_based::retrieve_fabric_like_data(
		fabric_based::FabricLikeLoaders::LegacyFabric,
		CURRENT_LEGACY_FABRIC_FORMAT_VERSION,
		minecraft_versions,
		uploaded_files,
		semaphore,
		LEGACY_FABRIC_MAVEN_URL,
		LEGACY_FABRIC_META_URL,
	)
	.await
}
