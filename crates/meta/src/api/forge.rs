use interpulse::api::minecraft::VersionManifest;

use crate::utils::*;

pub async fn retrieve_data(
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	crate::utils::forge_based::retrieve_forge_like_data(
		forge_based::ForgeLikeLoaders::Forge,
		minecraft_versions,
		uploaded_files,
		semaphore,
	)
	.await
}
