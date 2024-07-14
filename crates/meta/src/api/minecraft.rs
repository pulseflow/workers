use crate::utils::{download_file, fetch_json, format_url, sha1_async};
use crate::{Error, MirrorArtifact, UploadFile};
use dashmap::DashMap;
use interpulse::api::minecraft::{
	merge_partial_library, Library, VersionInfo, VersionManifest, VERSION_MANIFEST_URL,
};
use meta_patcher::patch::LibraryPatch;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tracing::instrument(skip(semaphore, upload_files, _mirror_artifacts))]
pub async fn fetch(
	semaphore: Arc<Semaphore>,
	upload_files: &DashMap<String, UploadFile>,
	_mirror_artifacts: &DashMap<String, MirrorArtifact>,
) -> Result<(), Error> {
	tracing::info!("fetching minecraft metadata!");
	let existing_manifest = fetch_json::<VersionManifest>(
		&format_url(&format!(
			"minecraft/v{}/manifest.json",
			interpulse::api::minecraft::CURRENT_FORMAT_VERSION
		)),
		&semaphore,
	)
	.await
	.ok();
	let mojang_manifest = fetch_json::<VersionManifest>(VERSION_MANIFEST_URL, &semaphore).await?;

	// TODO: experimental snapshots: https://github.com/PrismLauncher/meta/blob/main/meta/common/mojang-minecraft-experiments.json
	// TODO: old snapshots: https://github.com/PrismLauncher/meta/blob/main/meta/common/mojang-minecraft-old-snapshots.json

	let (fetch_versions, existing_versions) = if let Some(mut existing_manifest) = existing_manifest
	{
		let (mut fetch_versions, mut existing_versions) = (Vec::new(), Vec::new());

		for version in mojang_manifest.versions {
			if let Some(index) = existing_manifest
				.versions
				.iter()
				.position(|x| x.id == version.id)
			{
				let existing_version = existing_manifest.versions.remove(index);

				if existing_version
					.original_sha1
					.as_ref()
					.map(|x| x == &version.sha1)
					.unwrap_or(false)
				{
					existing_versions.push(existing_version);
				} else {
					fetch_versions.push(version);
				}
			} else {
				fetch_versions.push(version);
			}
		}

		(fetch_versions, existing_versions)
	} else {
		(mojang_manifest.versions, Vec::new())
	};

	if !fetch_versions.is_empty() {
		let version_manifests = futures::future::try_join_all(
			fetch_versions
				.iter()
				.map(|x| download_file(&x.url, Some(&x.sha1), &semaphore)),
		)
		.await?
		.into_iter()
		.map(|x| serde_json::from_slice(&x))
		.collect::<Result<Vec<VersionInfo>, serde_json::Error>>()?;

		// Patch libraries of Minecraft versions for M-series Mac Support, Better Linux Compatibility, etc
		let library_patches = fetch_library_patches()?;
		let patched_version_manifests = version_manifests
			.into_iter()
			.map(|mut x| {
				if !library_patches.is_empty() {
					let mut new_libraries = Vec::new();
					for library in x.libraries {
						let mut libs = patch_library(&library_patches, library);
						new_libraries.append(&mut libs)
					}
					x.libraries = new_libraries
				}

				x
			})
			.collect::<Vec<_>>();

		let serialized_version_manifests = patched_version_manifests
			.iter()
			.map(|x| serde_json::to_vec(x).map(bytes::Bytes::from))
			.collect::<Result<Vec<_>, serde_json::Error>>()?;
		let hashes_version_manifests = futures::future::try_join_all(
			serialized_version_manifests
				.iter()
				.map(|x| sha1_async(x.clone())),
		)
		.await?;

		let mut new_versions = patched_version_manifests
			.into_iter()
			.zip(serialized_version_manifests.into_iter())
			.zip(hashes_version_manifests.into_iter())
			.map(|((version, bytes), hash)| {
				let version_path = format!(
					"minecraft/v{}/versions/{}.json",
					interpulse::api::minecraft::CURRENT_FORMAT_VERSION,
					version.id
				);

				let url = format_url(&version_path);
				upload_files.insert(
					version_path,
					UploadFile {
						file: bytes,
						content_type: Some("application/json".to_string()),
					},
				);

				interpulse::api::minecraft::Version {
					original_sha1: fetch_versions
						.iter()
						.find(|x| x.id == version.id)
						.map(|x| x.sha1.clone()),
					id: version.id,
					type_: version.type_,
					url,
					time: version.time,
					release_time: version.release_time,
					sha1: hash,
					compliance_level: 1,
				}
			})
			.chain(existing_versions.into_iter())
			.collect::<Vec<_>>();

		new_versions.sort_by(|a, b| b.release_time.cmp(&a.release_time));

		let version_manifest_path = format!(
			"minecraft/v{}/manifest.json",
			interpulse::api::minecraft::CURRENT_FORMAT_VERSION
		);

		let new_manifest = VersionManifest {
			latest: mojang_manifest.latest,
			versions: new_versions,
		};

		upload_files.insert(
			version_manifest_path,
			UploadFile {
				file: bytes::Bytes::from(serde_json::to_vec(&new_manifest)?),
				content_type: Some("application/json".to_string()),
			},
		);
	}

	Ok(())
}

fn fetch_library_patches() -> Result<Vec<LibraryPatch>, Error> {
	let patches = include_bytes!("../../library.json");
	Ok(serde_json::from_slice(patches)?)
}

pub fn patch_library(patches: &Vec<LibraryPatch>, mut library: Library) -> Vec<Library> {
	let mut val = Vec::new();

	let actual_patches = patches
		.iter()
		.filter(|x| x.match_.contains(&library.name))
		.collect::<Vec<_>>();

	if !actual_patches.is_empty() {
		for patch in actual_patches {
			if let Some(override_) = &patch.override_ {
				library = merge_partial_library(override_.clone(), library);
			}

			if let Some(additional_libraries) = &patch.additional_libraries {
				for additional_library in additional_libraries {
					if patch.patch_additional_libraries.unwrap_or(false) {
						let mut libs = patch_library(patches, additional_library.clone());
						val.append(&mut libs)
					} else {
						val.push(additional_library.clone());
					}
				}
			}
		}

		val.push(library);
	} else {
		val.push(library);
	}

	val
}
