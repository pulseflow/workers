use interpulse::api::minecraft::VersionManifest;
use interpulse::api::modded::CURRENT_FABRIC_FORMAT_VERSION;

use crate::utils::*;

const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";
const FABRIC_MAVEN_URL: &str = "https://maven.fabricmc.net/";

pub async fn retrieve_data(
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	crate::utils::fabric_based::retrieve_fabric_like_data(
		fabric_based::FabricLikeLoaders::Fabric,
		CURRENT_FABRIC_FORMAT_VERSION,
		minecraft_versions,
		uploaded_files,
		semaphore,
		FABRIC_MAVEN_URL,
		FABRIC_META_URL,
	)
	.await
}
