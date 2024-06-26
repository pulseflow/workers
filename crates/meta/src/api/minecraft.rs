use interpulse::api::minecraft::{
	self, fetch_version_info, fetch_version_manifest, merge_partial_library, Library,
	VersionManifest, CURRENT_FORMAT_VERSION,
};
use interpulse::utils::get_hash;
use meta_patcher::patch::LibraryPatch;

use crate::utils::*;

pub async fn retrieve_data(
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<VersionManifest> {
	let old_manifest = fetch_version_manifest(Some(&format_url(&format!(
		"minecraft/v{}/manifest.json",
		CURRENT_FORMAT_VERSION,
	))))
	.await
	.ok();

	let mut manifest = fetch_version_manifest(None).await?;
	let cloned_manifest = Arc::new(Mutex::new(old_manifest.clone().unwrap_or(manifest.clone())));

	let patches = fetch_library_patches()?;
	let cloned_patches = Arc::new(&patches);

	let visited_assets_mutex = Arc::new(Mutex::new(Vec::new()));
	let uploaded_files_mutex = Arc::new(Mutex::new(Vec::new()));

	let now = Instant::now();

	let mut version_futures = Vec::new();

	for version in manifest.versions.iter_mut() {
		version_futures.push(async {
			let old_version = if let Some(old_manifest) = &old_manifest {
				old_manifest.versions.iter().find(|x| x.id == version.id)
			} else {
				None
			};

			if old_version.is_some() {
				return Ok(());
			}

			let visited_assets_mutex = Arc::clone(&visited_assets_mutex);
			let cloned_manifest_mutex = Arc::clone(&cloned_manifest);
			let uploaded_files_mutex = Arc::clone(&uploaded_files_mutex);
			let semaphore = Arc::clone(&semaphore);
			let patches = Arc::clone(&cloned_patches);
			let assets_hash = old_version.and_then(|x| x.assets_index_sha1.clone());

			async move {
				let mut upload_futures = Vec::new();
				let mut version_info = fetch_version_info(version).await?;

				fn patch_library(
					patches: &Vec<LibraryPatch>,
					mut library: Library,
				) -> Vec<Library> {
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
										let mut libs =
											patch_library(patches, additional_library.clone());
										val.append(&mut libs)
									} else {
										val.push(additional_library.clone());
									}
								}
							}
						}

						val.push(library)
					} else {
						val.push(library);
					}

					val
				}

				let mut new_libraries = Vec::new();
				for library in version_info.libraries.clone() {
					let mut libs = patch_library(&patches, library);
					new_libraries.append(&mut libs)
				}
				version_info.libraries = new_libraries;

				let version_info_hash =
					get_hash(bytes::Bytes::from(serde_json::to_vec(&version_info)?)).await?;
				let version_path = format!(
					"minecraft/v{}/versions/{}.json",
					CURRENT_FORMAT_VERSION, version.id
				);
				let assets_path = format!(
					"minecraft/v{}/assets/{}.json",
					CURRENT_FORMAT_VERSION, version_info.asset_index.id
				);
				let assets_index_url = version_info.asset_index.url.clone();

				{
					let mut cloned_manifest = cloned_manifest_mutex.lock().await;
					if let Some(position) = cloned_manifest
						.versions
						.iter()
						.position(|x| version.id == x.id)
					{
						cloned_manifest.versions[position].url = format_url(&version_path);
						cloned_manifest.versions[position].assets_index_sha1 =
							Some(version_info.asset_index.sha1.clone());
						cloned_manifest.versions[position].assets_index_url =
							Some(format_url(&assets_path));
						cloned_manifest.versions[position].sha1 = version_info_hash;
					} else {
						cloned_manifest.versions.insert(
							0,
							minecraft::Version {
								id: version_info.id.clone(),
								type_: version_info.type_,
								url: format_url(&version_path),
								time: version_info.time,
								release_time: version_info.release_time,
								sha1: version_info_hash,
								compliance_level: 1,
								assets_index_url: Some(version_info.asset_index.sha1.clone()),
								assets_index_sha1: Some(version_info.asset_index.sha1.clone()),
							},
						)
					}
				}

				let mut download_assets = false;

				{
					let mut visited_assets = visited_assets_mutex.lock().await;
					if !visited_assets.contains(&version_info.asset_index.id) {
						if let Some(assets_hash) = assets_hash {
							if version_info.asset_index.sha1 != assets_hash {
								download_assets = true;
							}
						} else {
							download_assets = true;
						}
					}

					if download_assets {
						visited_assets.push(version_info.asset_index.id.clone());
					}
				}

				if download_assets {
					let assets_index = download_file(
						&assets_index_url,
						Some(&version_info.asset_index.sha1),
						semaphore.clone(),
					)
					.await?;

					{
						upload_futures.push(upload_file_to_bucket(
							assets_path,
							assets_index.to_vec(),
							Some("application/json".to_string()),
							uploaded_files_mutex.as_ref(),
							semaphore.clone(),
						));
					}
				}

				{
					upload_futures.push(upload_file_to_bucket(
						version_path,
						serde_json::to_vec(&version_info)?,
						Some("application/json".to_string()),
						uploaded_files_mutex.as_ref(),
						semaphore.clone(),
					));
				}

				futures::future::try_join_all(upload_futures).await?;

				Ok::<(), crate::Error>(())
			}
			.await?;

			Ok::<(), crate::Error>(())
		})
	}

	{
		let mut versions = version_futures.into_iter().peekable();
		let mut chunk_index = 0;
		while versions.peek().is_some() {
			let now = Instant::now();
			let chunk: Vec<_> = versions.by_ref().take(100).collect();
			futures::future::try_join_all(chunk).await?;
			chunk_index += 1;
			let elapsed = now.elapsed();
			info!("chunk {} elapsed: {:.2?}", chunk_index, elapsed);
		}
	}

	upload_file_to_bucket(
		format!("minecraft/v{}/manifest.json", CURRENT_FORMAT_VERSION),
		serde_json::to_vec(&*cloned_manifest.lock().await)?,
		Some("application/json".to_string()),
		uploaded_files_mutex.as_ref(),
		semaphore,
	)
	.await?;

	if let Ok(uploaded_files_mutex) = Arc::try_unwrap(uploaded_files_mutex) {
		uploaded_files.extend(uploaded_files_mutex.into_inner());
	}

	let elapsed = now.elapsed();
	info!("elapsed: {:.2?}", elapsed);

	Ok(Arc::try_unwrap(cloned_manifest)
		.map_err(|_| crate::Error::ArcError)?
		.into_inner())
}

fn fetch_library_patches() -> crate::Result<Vec<meta_patcher::patch::LibraryPatch>> {
	let patches = include_bytes!("../../library.json");
	Ok(serde_json::from_slice(patches)?)
}
