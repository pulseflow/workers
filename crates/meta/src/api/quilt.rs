use interpulse::api::{minecraft::VersionManifest, modded::CURRENT_QUILT_FORMAT_VERSION};

use crate::utils::*;

const QUILT_META_URL: &str = "https://meta.quiltmc.org/v2";
const QUILT_MAVEN_URL: &str = "https://maven.quiltmc.org/";

pub async fn retrieve_data(
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	crate::utils::fabric_based::retrieve_fabric_like_data(
		fabric_based::FabricLikeLoaders::Quilt,
		CURRENT_QUILT_FORMAT_VERSION,
		minecraft_versions,
		uploaded_files,
		semaphore,
		QUILT_MAVEN_URL,
		QUILT_META_URL,
	)
	.await
}
