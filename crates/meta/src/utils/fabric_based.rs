use interpulse::api::minecraft::{Library, VersionManifest};
use interpulse::api::modded::{
	LoaderVersion, Manifest, PartialVersionInfo, Version, CURRENT_FABRIC_FORMAT_VERSION,
	CURRENT_LEGACY_FABRIC_FORMAT_VERSION, CURRENT_QUILT_FORMAT_VERSION, DUMMY_REPLACE_STRING,
};
use interpulse::utils::get_path_from_artifact;

use crate::utils::*;

#[derive(Clone)]
pub enum FabricLikeLoaders {
	Fabric,
	Quilt,
	LegacyFabric,
}

impl FabricLikeLoaders {
	pub fn as_str(&self) -> &'static str {
		match self {
			FabricLikeLoaders::Fabric => "fabric",
			FabricLikeLoaders::Quilt => "quilt",
			FabricLikeLoaders::LegacyFabric => "legacy-fabric",
		}
	}

	pub fn as_format(&self) -> usize {
		match self {
			FabricLikeLoaders::Fabric => CURRENT_FABRIC_FORMAT_VERSION,
			FabricLikeLoaders::Quilt => CURRENT_QUILT_FORMAT_VERSION,
			FabricLikeLoaders::LegacyFabric => CURRENT_LEGACY_FABRIC_FORMAT_VERSION,
		}
	}

	pub fn as_maven_url(&self) -> &'static str {
		match self {
			FabricLikeLoaders::Fabric => "https://maven.fabricmc.net/",
			FabricLikeLoaders::Quilt => "https://maven.quiltmc.org/",
			FabricLikeLoaders::LegacyFabric => {
				"https://repo.legacyfabric.net/repository/legacyfabric/"
			}
		}
	}

	pub fn as_meta_url(&self) -> &'static str {
		match self {
			FabricLikeLoaders::Fabric => "https://meta.fabricmc.net/v2",
			FabricLikeLoaders::Quilt => "https://meta.quiltmc.org/v2",
			FabricLikeLoaders::LegacyFabric => "https://meta.legacyfabric.net/v2",
		}
	}
}

pub async fn retrieve_fabric_like_data(
	loader_name: FabricLikeLoaders,
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	let list = fetch_fabric_like_versions::<FabricLikeVersions>(
		None,
		semaphore.clone(),
		loader_name.as_meta_url(),
	)
	.await?;
	let old_manifest = interpulse::api::modded::fetch_manifest(&format_url(&format!(
		"{}/v{}/manifest.json",
		loader_name.as_str(),
		loader_name.as_format(),
	)))
	.await
	.ok();

	let mut versions = if let Some(old_manifest) = old_manifest {
		old_manifest.game_versions
	} else {
		Vec::new()
	};
	let loaders_mutex = RwLock::new(Vec::new());

	{
		let mut loaders = loaders_mutex.write().await;
		for (index, loader) in list.loader.iter().enumerate() {
			if versions.iter().any(|x| {
				x.id == DUMMY_REPLACE_STRING && x.loaders.iter().any(|x| x.id == loader.version)
			}) {
				loaders.push((
					Box::new(loader.stable.unwrap_or(false)),
					loader.version.clone(),
					Box::new(index == 0),
				))
			}
		}
	}

	const DUMMY_GAME_VERSION: &str = "1.19.4-rc2";

	let loader_version_mutex = Mutex::new(Vec::new());
	let uploaded_files_mutex = Arc::new(Mutex::new(Vec::new()));

	let loader_versions =
		futures::future::try_join_all(loaders_mutex.read().await.clone().into_iter().map(
			|(stable, loader, skip_upload)| async {
				let version = fetch_fabric_like_version(
					DUMMY_GAME_VERSION,
					&loader,
					semaphore.clone(),
					loader_name.as_meta_url(),
				)
				.await?;
				Ok::<(Box<bool>, String, PartialVersionInfo, Box<bool>), crate::Error>((
					stable,
					loader,
					version,
					skip_upload,
				))
			},
		))
		.await?;

	let visited_artifacts_mutex = Arc::new(Mutex::new(Vec::new()));
	futures::future::try_join_all(loader_versions.into_iter().map(
		|(stable, loader, version, skip_upload)| async {
			let loader_name = loader_name.clone();
			let libs = futures::future::try_join_all(version.libraries.clone().into_iter().map(
				|mut lib| async {
					let loader_name = loader_name.clone();
					{
						let mut visited_assets = visited_artifacts_mutex.lock().await;
						if visited_assets.contains(&lib.name) {
							lib.name = lib.name.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING);
							lib.url = Some(format_url("maven/"));

							return Ok(lib);
						} else {
							visited_assets.push(lib.name.clone())
						}
					}

					if lib.name.contains(DUMMY_GAME_VERSION) {
						lib.name = lib.name.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING);
						futures::future::try_join_all(list.game.clone().into_iter().map(
							|game_version| async {
								let loader_name = loader_name.clone();
								let semaphore = semaphore.clone();
								let uploaded_files_mutex = uploaded_files_mutex.clone();
								let lib_name = lib.name.clone();
								let lib_url = lib.url.clone();

								async move {
									let artifact_path = get_path_from_artifact(
										&lib_name
											.replace(DUMMY_REPLACE_STRING, &game_version.version),
									)?;
									let artifact = download_file(
										&format!(
											"{}{}",
											lib_url.unwrap_or_else(|| loader_name
												.as_maven_url()
												.to_string()),
											artifact_path
										),
										None,
										semaphore.clone(),
									)
									.await?;
									upload_file_to_bucket(
										format!("{}/{}", "maven", artifact_path),
										artifact.to_vec(),
										Some("application/java-archive".to_string()),
										&uploaded_files_mutex,
										semaphore.clone(),
									)
									.await?;
									Ok::<(), crate::Error>(())
								}
								.await?;

								Ok::<(), crate::Error>(())
							},
						))
						.await?;

						lib.url = Some(format_url("maven/"));

						return Ok(lib);
					}

					let artifact_path = get_path_from_artifact(&lib.name)?;
					let artifact = download_file(
						&format!(
							"{}{}",
							lib.url
								.unwrap_or_else(|| loader_name.as_maven_url().to_string()),
							artifact_path
						),
						None,
						semaphore.clone(),
					)
					.await?;
					lib.url = Some(format_url("maven/"));
					upload_file_to_bucket(
						format!("{}/{}", "maven", artifact_path),
						artifact.to_vec(),
						Some("application/java-archive".to_string()),
						&uploaded_files_mutex,
						semaphore.clone(),
					)
					.await?;

					Ok::<Library, crate::Error>(lib)
				},
			))
			.await?;

			if async move { *skip_upload }.await {
				return Ok::<(), crate::Error>(());
			}

			let version = Arc::new(version);
			let version_path = format!(
				"{}/v{}/versions/{}.json",
				loader_name.as_str(),
				loader_name.as_format(),
				&loader
			);
			upload_file_to_bucket(
				version_path.clone(),
				serde_json::to_vec(&PartialVersionInfo {
					arguments: version.arguments.as_ref().cloned(),
					id: version
						.id
						.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING)
						.clone(),
					main_class: version.main_class.as_ref().cloned(),
					release_time: version.release_time,
					time: version.time,
					type_: version.type_,
					inherits_from: version
						.inherits_from
						.replace(DUMMY_GAME_VERSION, DUMMY_REPLACE_STRING),
					libraries: libs,
					minecraft_arguments: version.minecraft_arguments.as_ref().cloned(),
					processors: None,
					data: None,
				})?,
				Some("application/json".to_string()),
				&uploaded_files_mutex,
				semaphore.clone(),
			)
			.await?;

			{
				let mut loader_version_map = loader_version_mutex.lock().await;
				async move {
					loader_version_map.push(LoaderVersion {
						id: loader.to_string(),
						url: format_url(&version_path),
						stable: *stable,
					});
				}
				.await;
			}

			Ok::<(), crate::Error>(())
		},
	))
	.await?;

	let mut loader_version_mutex = loader_version_mutex.into_inner();
	if !loader_version_mutex.is_empty() {
		if let Some(version) = versions.iter_mut().find(|x| x.id == DUMMY_REPLACE_STRING) {
			version.loaders.append(&mut loader_version_mutex);
		} else {
			versions.push(Version {
				id: DUMMY_REPLACE_STRING.to_string(),
				stable: true,
				loaders: loader_version_mutex,
			});
		}
	}

	for version in &list.game {
		if !versions.iter().any(|x| x.id == version.version) {
			versions.push(Version {
				id: version.version.clone(),
				stable: version.stable,
				loaders: vec![],
			});
		}
	}

	versions.sort_by(|x, y| {
		minecraft_versions
			.versions
			.iter()
			.position(|z| x.id == z.id)
			.unwrap_or_default()
			.cmp(
				&minecraft_versions
					.versions
					.iter()
					.position(|z| y.id == z.id)
					.unwrap_or_default(),
			)
	});

	for version in &mut versions {
		version.loaders.sort_by(|x, y| {
			list.loader
				.iter()
				.position(|z| x.id == *z.version)
				.unwrap_or_default()
				.cmp(
					&list
						.loader
						.iter()
						.position(|z| y.id == z.version)
						.unwrap_or_default(),
				)
		})
	}

	upload_file_to_bucket(
		format!(
			"{}/v{}/manifest.json",
			loader_name.as_str(),
			loader_name.as_format()
		),
		serde_json::to_vec(&Manifest {
			game_versions: versions,
		})?,
		Some("application/json".to_string()),
		&uploaded_files_mutex,
		semaphore,
	)
	.await?;

	if let Ok(uploaded_files_mutex) = Arc::try_unwrap(uploaded_files_mutex) {
		uploaded_files.extend(uploaded_files_mutex.into_inner());
	}

	Ok(())
}

pub async fn fetch_fabric_like_version(
	version_number: &str,
	loader_version: &str,
	semaphore: Arc<Semaphore>,
	meta_url: &str,
) -> crate::Result<PartialVersionInfo> {
	Ok(serde_json::from_slice(
		&download_file(
			&format!(
				"{}/versions/loader/{}/{}/profile/json",
				meta_url, version_number, loader_version
			),
			None,
			semaphore,
		)
		.await?,
	)?)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Versions of fabric components
pub struct FabricLikeVersions {
	/// Versions of Minecraft that fabric supports
	pub game: Vec<FabricLikeGameVersion>,
	/// Available versions of the fabric loader
	pub loader: Vec<FabricLikeLoaderVersion>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// A version of Minecraft that fabric supports
pub struct FabricLikeGameVersion {
	/// The version number of the game
	pub version: String,
	/// Whether the Minecraft version is stable or not
	pub stable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// A version of the fabric loader
pub struct FabricLikeLoaderVersion {
	/// The separator to get the build number
	pub separator: String,
	/// The build number
	pub build: u32,
	/// The maven artifact
	pub maven: String,
	/// The version number of the fabric loader
	pub version: String,
	/// Whether the loader is stable or not
	#[serde(skip_serializing_if = "Option::is_none")]
	pub stable: Option<bool>,
}

pub async fn fetch_fabric_like_versions<T: for<'a> serde::Deserialize<'a>>(
	url: Option<&str>,
	semaphore: Arc<Semaphore>,
	meta_url: &str,
) -> crate::Result<T> {
	Ok(serde_json::from_slice(
		&download_file(
			url.unwrap_or(&*format!("{}/versions", meta_url)),
			None,
			semaphore,
		)
		.await?,
	)?)
}
