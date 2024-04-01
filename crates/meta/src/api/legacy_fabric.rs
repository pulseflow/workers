use interpulse::api::minecraft::VersionManifest;

use crate::utils::*;

// nightmare nightmare nightmare nightmare
pub async fn retrieve_data(
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	crate::utils::fabric_based::retrieve_fabric_like_data(
		fabric_based::FabricLikeLoaders::LegacyFabric,
		minecraft_versions,
		uploaded_files,
		semaphore,
	)
	.await
}
