use crate::utils::prelude::*;
use interpulse::api::modded::{DUMMY_REPLACE_STRING, Manifest, PartialVersionInfo};

#[tracing::instrument(skip(semaphore, upload_files, mirror_artifacts))]
pub async fn fetch_fabric(
	semaphore: Arc<Semaphore>,
	upload_files: &crate::UploadFiles,
	mirror_artifacts: &crate::MirrorArtifacts,
) -> crate::utils::Result<()> {
	fetch(
		interpulse::api::modded::CURRENT_FABRIC_FORMAT_VERSION,
		"fabric",
		"https://meta.fabricmc.net/v2",
		"https://maven.fabricmc.net/",
		&[],
		semaphore,
		upload_files,
		mirror_artifacts,
	)
	.await
}

#[tracing::instrument(skip(semaphore, upload_files, mirror_artifacts))]
pub async fn fetch_legacy_fabric(
	semaphore: Arc<Semaphore>,
	upload_files: &crate::UploadFiles,
	mirror_artifacts: &crate::MirrorArtifacts,
) -> crate::utils::Result<()> {
	fetch(
		interpulse::api::modded::CURRENT_LEGACY_FABRIC_FORMAT_VERSION,
		"legacy-fabric",
		"https://meta.legacyfabric.net/v2",
		"https://repo.legacyfabric.net/repository/legacyfabric/",
		&[],
		semaphore,
		upload_files,
		mirror_artifacts,
	)
	.await
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip(semaphore, upload_files, mirror_artifacts))]
pub async fn fetch_quilt(
	semaphore: Arc<Semaphore>,
	upload_files: &crate::UploadFiles,
	mirror_artifacts: &crate::MirrorArtifacts,
) -> crate::utils::Result<()> {
	fetch(
		interpulse::api::modded::CURRENT_QUILT_FORMAT_VERSION,
		"quilt",
		"https://meta.quiltmc.org/v3",
		"https://maven.quiltmc.org/repository/release/",
		&["0.17.5-beta.4"], // invalid library coordinates
		semaphore,
		upload_files,
		mirror_artifacts,
	)
	.await
}

#[tracing::instrument(skip(semaphore, upload_files, mirror_artifacts))]
#[allow(clippy::too_many_arguments)]
async fn fetch(
	format_version: usize,
	mod_loader: &str,
	meta_url: &str,
	maven_url: &str,
	skip_versions: &[&str],
	semaphore: Arc<Semaphore>,
	upload_files: &crate::UploadFiles,
	mirror_artifacts: &crate::MirrorArtifacts,
) -> crate::utils::Result<()> {
	const DUMMY_GAME_VERSION: &str = "1.21";
	tracing::info!("fetching fabric mod loader metadata for {mod_loader}!");
	let existing_manifest = crate::utils::fetch_json::<Manifest>(
		&crate::utils::format_url(&format!("{mod_loader}/v{format_version}/manifest.json",)),
		&semaphore,
	)
	.await
	.ok();
	let fabric_manifest =
		crate::utils::fetch_json::<FabricVersions>(&format!("{meta_url}/versions"), &semaphore)
			.await?;

	// we can check our fabric manifest and compare it to fabric's to avoid unnecessary processing
	let (fetch_fabric_versions, fetch_intermediary_versions) =
		if let Some(existing_manifest) = existing_manifest {
			let (mut fetch_versions, mut fetch_intermediary_versions) = (Vec::new(), Vec::new());

			for version in &fabric_manifest.loader {
				if !existing_manifest
					.game_versions
					.iter()
					.any(|x| x.loaders.iter().any(|x| x.id == version.version))
					&& !skip_versions.contains(&&*version.version)
				{
					fetch_versions.push(version);
				}
			}

			for version in &fabric_manifest.intermediary {
				if !existing_manifest
					.game_versions
					.iter()
					.any(|x| x.id == version.version)
					&& fabric_manifest
						.game
						.iter()
						.any(|x| x.version == version.version)
				{
					fetch_intermediary_versions.push(version);
				}
			}

			(fetch_versions, fetch_intermediary_versions)
		} else {
			(
				fabric_manifest
					.loader
					.iter()
					.filter(|x| !skip_versions.contains(&&*x.version))
					.collect(),
				fabric_manifest.intermediary.iter().collect(),
			)
		};

	if !fetch_intermediary_versions.is_empty() {
		for x in &fetch_intermediary_versions {
			crate::utils::insert_mirrored_artifact(
				&x.maven,
				None,
				vec![maven_url.to_string()],
				false,
				mirror_artifacts,
			)?;
		}
	}

	if !fetch_fabric_versions.is_empty() {
		let fabric_version_manifest_urls = fetch_fabric_versions
			.iter()
			.map(|x| {
				format!(
					"{}/versions/loader/{}/{}/profile/json",
					meta_url, DUMMY_GAME_VERSION, x.version
				)
			})
			.collect::<Vec<_>>();
		let fabric_version_manifests = futures::future::try_join_all(
			fabric_version_manifest_urls
				.iter()
				.map(|x| crate::utils::download_file(x, None, &semaphore)),
		)
		.await?
		.into_iter()
		.map(|x| serde_json::from_slice(&x))
		.collect::<Result<Vec<PartialVersionInfo>, serde_json::Error>>()?;

		let patched_version_manifests = fabric_version_manifests
			.into_iter()
			.map(|mut version_info| {
				for lib in &mut version_info.libraries {
					let new_name = lib.name.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING);

					// `net.minecraft.launchwrapper:1.12` isn't present on fabric's maven server.
					// we hard code this to fetch it from mojang's servers
					if &*lib.name == "net.minecraft:launchwrapper:1.12" {
						lib.url = Some("https://libraries.minecraft.net/".to_string());
					}

					// if a library isn't an intermediary, we can add it to `mirror_artifacts`
					// to mirror on the s3 server, for redundancy
					if lib.name == new_name {
						crate::utils::insert_mirrored_artifact(
							&new_name,
							None,
							vec![lib.url.clone().unwrap_or_else(|| maven_url.to_string())],
							false,
							mirror_artifacts,
						)?;
					} else {
						lib.name = new_name;
					}

					lib.url = Some(crate::utils::format_url("maven/"));
				}

				version_info.id = version_info
					.id
					.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING);
				version_info.inherits_from = version_info
					.inherits_from
					.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING);

				Ok(version_info)
			})
			.collect::<crate::utils::Result<Vec<_>>>()?;
		let serialized_version_manifests = patched_version_manifests
			.iter()
			.map(|x| serde_json::to_vec(x).map(Bytes::from))
			.collect::<Result<Vec<_>, serde_json::Error>>()?;

		serialized_version_manifests
			.into_iter()
			.enumerate()
			.for_each(|(index, bytes)| {
				let loader = fetch_fabric_versions[index];

				let version_path = format!(
					"{mod_loader}/v{format_version}/versions/{}.json",
					loader.version
				);

				upload_files.insert(
					version_path,
					crate::utils::UploadFile {
						file: bytes,
						content_type: Some("application/json".to_string()),
					},
				);
			});
	}

	if !fetch_fabric_versions.is_empty() || !fetch_intermediary_versions.is_empty() {
		let fabric_manifest_path = format!("{mod_loader}/v{format_version}/manifest.json",);

		let loader_versions = interpulse::api::modded::Version {
			id: DUMMY_REPLACE_STRING.to_string(),
			stable: true,
			loaders: fabric_manifest
				.loader
				.into_iter()
				.map(|x| {
					let version_path =
						format!("{mod_loader}/v{format_version}/versions/{}.json", x.version);

					interpulse::api::modded::LoaderVersion {
						id: x.version,
						url: crate::utils::format_url(&version_path),
						stable: x.stable,
					}
				})
				.collect(),
		};

		let manifest =
			Manifest {
				game_versions: std::iter::once(loader_versions)
					.chain(fabric_manifest.game.into_iter().map(|x| {
						interpulse::api::modded::Version {
							id: x.version,
							stable: x.stable,
							loaders: vec![],
						}
					}))
					.collect(),
			};

		upload_files.insert(
			fabric_manifest_path,
			crate::utils::UploadFile {
				file: Bytes::from(serde_json::to_vec(&manifest)?),
				content_type: Some("application/json".to_string()),
			},
		);
	}

	Ok(())
}

#[derive(Deserialize, Debug, Clone)]
struct FabricVersions {
	pub loader: Vec<FabricLoaderVersion>,
	pub game: Vec<FabricGameVersion>,
	#[serde(alias = "hashed")]
	pub intermediary: Vec<FabricIntermediaryVersion>,
}

#[derive(Deserialize, Debug, Clone)]
struct FabricLoaderVersion {
	// pub separator: String,
	// pub build: u32,
	// pub maven: String,
	pub version: String,
	#[serde(default)]
	pub stable: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct FabricIntermediaryVersion {
	pub maven: String,
	pub version: String,
}

#[derive(Deserialize, Debug, Clone)]
struct FabricGameVersion {
	pub version: String,
	pub stable: bool,
}
