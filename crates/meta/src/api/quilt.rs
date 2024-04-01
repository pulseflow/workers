use interpulse::api::minecraft::VersionManifest;

use crate::utils::*;
pub async fn retrieve_data(
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	crate::utils::fabric_based::retrieve_fabric_like_data(
		fabric_based::FabricLikeLoaders::Quilt,
		minecraft_versions,
		uploaded_files,
		semaphore,
	)
	.await
}
