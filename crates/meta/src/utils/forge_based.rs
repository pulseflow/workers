use std::{collections::HashMap, io::Read};

use chrono::{DateTime, Utc};
use interpulse::{
	api::{
		minecraft::{Argument, ArgumentType, Library, VersionManifest, VersionType},
		modded::{
			fetch_manifest, LoaderVersion, Manifest, PartialVersionInfo, Processor, SidedDataEntry,
			CURRENT_FORGE_FORMAT_VERSION, CURRENT_NEOFORGE_FORMAT_VERSION,
		},
	},
	utils::get_path_from_artifact,
};
use semver::{Version, VersionReq};

use crate::utils::*;

lazy_static::lazy_static! {
	static ref FORGE_MANIFEST_V1_QUERY: VersionReq =
		VersionReq::parse(">=8.0.684, <23.5.2851").unwrap();
	static ref FORGE_MANIFEST_V2_QUERY_P1: VersionReq =
		VersionReq::parse(">=23.5.2851, <31.2.52").unwrap();
	static ref FORGE_MANIFEST_V2_QUERY_P2: VersionReq =
		VersionReq::parse(">=32.0.1, <37.0.0").unwrap();
	static ref FORGE_MANIFEST_V3_QUERY: VersionReq =
		VersionReq::parse(">=37.0.0").unwrap();
}

#[derive(PartialEq, Clone)]
pub enum ForgeLikeLoaders {
	Forge,
	Neo,
}

impl ForgeLikeLoaders {
	pub fn as_str(&self) -> &'static str {
		match self {
			ForgeLikeLoaders::Forge => "forge",
			ForgeLikeLoaders::Neo => "neo",
		}
	}

	pub fn as_format(&self) -> usize {
		match self {
			ForgeLikeLoaders::Forge => CURRENT_FORGE_FORMAT_VERSION,
			ForgeLikeLoaders::Neo => CURRENT_NEOFORGE_FORMAT_VERSION,
		}
	}
}

pub async fn retrieve_forge_like_data(
	loader_name: ForgeLikeLoaders,
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	let maven_metadata = match loader_name {
		ForgeLikeLoaders::Forge => fetch_forge_metadata(None, semaphore.clone()).await?,
		ForgeLikeLoaders::Neo => fetch_neo_metadata(semaphore.clone()).await?,
	};
	let old_manifest = fetch_manifest(&format_url(&format!(
		"{}/v{}/manifest.json",
		loader_name.as_str(),
		loader_name.as_format(),
	)))
	.await
	.ok();

	let old_versions = Arc::new(Mutex::new(if let Some(old_manifest) = old_manifest {
		old_manifest.game_versions
	} else {
		Vec::new()
	}));

	let versions = Arc::new(Mutex::new(Vec::new()));
	let visited_assets_mutex = Arc::new(Mutex::new(Vec::new()));
	let uploaded_files_mutex = Arc::new(Mutex::new(Vec::new()));
	let mut version_futures = Vec::new();

	for (minecraft_version, loader_versions) in maven_metadata.clone() {
		// Vec<(full_loader_version, parsed_version, is_new_forge)>
		let mut loaders = Vec::new();

		for (full, loader_version, new_forge) in loader_versions {
			let version = Version::parse(&loader_version)?;

			if loader_name == ForgeLikeLoaders::Forge {
				if FORGE_MANIFEST_V1_QUERY.matches(&version)
					|| FORGE_MANIFEST_V2_QUERY_P1.matches(&version)
					|| FORGE_MANIFEST_V2_QUERY_P2.matches(&version)
					|| FORGE_MANIFEST_V3_QUERY.matches(&version)
				{
					loaders.push((full, version, new_forge.to_string()))
				}
			} else {
				loaders.push((full, version, new_forge.to_string()))
			}
		}

		if !loaders.is_empty() {
			version_futures.push(async {
				let mut loaders_versions = Vec::new();

				{
					let loaders_futures = loaders.into_iter().map(|(loader_version_full, parsed_version, new_forge)| async {
						let versions_mutex = Arc::clone(&old_versions);
						let visited_assets = Arc::clone(&visited_assets_mutex);
						let uploaded_files_mutex = Arc::clone(&uploaded_files_mutex);
						let semaphore = Arc::clone(&semaphore);
						let minecraft_version = minecraft_version.clone();
						let loader_name = loader_name.clone();

						async move {
							if loader_name == ForgeLikeLoaders::Forge {
								/// fuck you forge
								const WHITELIST: &[&str] = &[
									// data field is [] even though the type is a map (i cba)
									"1.12.2-14.23.5.2851",
									// malformed archives
									"1.6.1-8.9.0.749",
                                	"1.6.1-8.9.0.751",
                                	"1.6.4-9.11.1.960",
                                	"1.6.4-9.11.1.961",
                                	"1.6.4-9.11.1.963",
                                	"1.6.4-9.11.1.964",
								];

								if WHITELIST.contains(&&*loader_version_full) {
									return Ok(None);
								}
							}

							{
								let versions = versions_mutex.lock().await;
								let version = versions.iter().find(|x|
									x.id == minecraft_version).and_then(|x| x.loaders.iter().find(|x| x.id == loader_version_full));

								if let Some(version) = version {
									return Ok::<Option<LoaderVersion>, crate::Error>(Some(version.clone()));
								}
							}

							info!("Forge - Installer Start {}", loader_version_full.clone());

							let bytes = match loader_name {
								ForgeLikeLoaders::Forge => download_file(&format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{0}/forge-{0}-installer.jar", loader_version_full), None, semaphore.clone()).await?,
								ForgeLikeLoaders::Neo => download_file(&format!("https://maven.neoforged.net/net/neoforged/{1}/{0}/{1}-{0}-installer.jar", loader_version_full, if &*new_forge == "true" { "neoforge" } else { "forge" }), None, semaphore.clone()).await?
							};

							let reader = std::io::Cursor::new(bytes);

							if let Ok(archive) = zip::ZipArchive::new(reader) {
								if loader_name == ForgeLikeLoaders::Forge && FORGE_MANIFEST_V1_QUERY.matches(&parsed_version) {
									let mut archive_clone = archive.clone();
									let profile = tokio::task::spawn_blocking(move || {
										let mut install_profile = archive_clone.by_name("install_profile.json")?;
										let mut contents = String::new();

										install_profile.read_to_string(&mut contents)?;
										Ok::<ForgeInstallerProfileV1, crate::Error>(serde_json::from_str::<ForgeInstallerProfileV1>(&contents)?)
									}).await??;

									let mut archive_clone = archive.clone();
									let file_path = profile.install.file_path.clone();
									let forge_universal_bytes = tokio::task::spawn_blocking(move || {
										let mut forge_universal_file = archive_clone.by_name(&file_path)?;
										let mut forge_universal = Vec::new();
										forge_universal_file.read_to_end(&mut forge_universal)?;

										Ok::<bytes::Bytes, crate::Error>(bytes::Bytes::from(forge_universal))
									}).await??;
									let forge_universal_path = profile.install.path.clone();

									let now = Instant::now();
									let libs = futures::future::try_join_all(profile.version_info.libraries.into_iter().map(|mut lib| async {
										if let Some(url) = lib.url {
											{
												let mut visited_assets = visited_assets.lock().await;
												if visited_assets.contains(&lib.name) {
													lib.url = Some(format_url("maven/"));

													return Ok::<Library, crate::Error>(lib);
												} else {
													visited_assets.push(lib.name.clone())
												}
											}

											let artifact_path = get_path_from_artifact(&lib.name)?;
											let artifact = if lib.name == forge_universal_path {
												forge_universal_bytes.clone()
											} else {
												let mirrors = vec![&*url, "https://maven.creeperhost.net/", "https://libraries.minecraft.net/"];
												download_file_mirrors(&artifact_path, &mirrors, None, semaphore.clone()).await?
											};

											lib.url = Some(format_url("maven/"));

											upload_file_to_bucket(
												format!("{}/{}", "maven", artifact_path),
												artifact.to_vec(),
												Some("application/java-archive".to_string()),
												uploaded_files_mutex.as_ref(),
												semaphore.clone(),
											).await?;
										}

										Ok::<Library, crate::Error>(lib)
									})).await?;

									let elapsed = now.elapsed();
									info!("elapsed lib dl: {:.2?}", elapsed);

									let new_profile = PartialVersionInfo {
										id: profile.version_info.id,
										inherits_from: profile.install.minecraft,
										release_time: profile.version_info.release_time,
										time: profile.version_info.time,
										main_class: profile.version_info.main_class,
										minecraft_arguments: profile.version_info.minecraft_arguments.clone(),
										arguments: profile.version_info.minecraft_arguments.map(|x| [(ArgumentType::Game, x.split(' ').map(|x| Argument::Normal(x.to_string())).collect())].iter().cloned().collect()),
										libraries: libs,
										type_: profile.version_info.type_,
										data: None,
										processors: None
									};

									let version_path = format!(
										"forge/v{}/versions/{}.json",
										CURRENT_FORGE_FORMAT_VERSION,
										new_profile.id
									);

									upload_file_to_bucket(
										version_path.clone(),
										serde_json::to_vec(&new_profile)?,
										Some("application/json".to_string()),
										uploaded_files_mutex.as_ref(),
										semaphore.clone(),
									).await?;

									return Ok(Some(LoaderVersion {
										id: loader_version_full,
										url: format_url(&version_path),
										stable: false,
									}));
								} else if (loader_name == ForgeLikeLoaders::Forge && (FORGE_MANIFEST_V2_QUERY_P1.matches(&parsed_version) || FORGE_MANIFEST_V2_QUERY_P2.matches(&parsed_version) || FORGE_MANIFEST_V3_QUERY.matches(&parsed_version))) || (loader_name == ForgeLikeLoaders::Neo) {
									let mut archive_clone = archive.clone();
									let mut profile = tokio::task::spawn_blocking(move || {
										let mut install_profile = archive_clone.by_name("install_profile.json")?;
										let mut contents = String::new();

										install_profile.read_to_string(&mut contents)?;

										Ok::<ForgeInstallerProfileV2, crate::Error>(serde_json::from_str::<ForgeInstallerProfileV2>(&contents)?)
									}).await??;

									let mut archive_clone = archive.clone();
									let version_info = tokio::task::spawn_blocking(move || {
										let mut install_profile = archive_clone.by_name("version.json")?;
										let mut contents = String::new();

										install_profile.read_to_string(&mut contents)?;

										Ok::<PartialVersionInfo, crate::Error>(serde_json::from_str::<PartialVersionInfo>(&contents)?)
									}).await??;

									let mut libs: Vec<Library> = version_info.libraries.into_iter().chain(profile.libraries.into_iter().map(|x| Library {
										downloads: x.downloads,
										extract: x.extract,
										name: x.name,
										url: x.url,
										natives: x.natives,
										rules: x.rules,
										checksums: x.checksums,
										include_in_classpath: false
									})).collect();

									let mut local_libs: HashMap<String, bytes::Bytes> = HashMap::new();

									for lib in &libs {
										if lib.downloads.as_ref().and_then(|x| x.artifact.as_ref().map(|x| x.url.is_empty())).unwrap_or(false) {
											let mut archive_clone = archive.clone();
											let lib_name_clone = lib.name.clone();

											let lib_bytes = tokio::task::spawn_blocking(move || {
												let mut lib_file = archive_clone.by_name(&format!("maven/{}", get_path_from_artifact(&lib_name_clone)?))?;
												let mut lib_bytes = Vec::new();
												lib_file.read_to_end(&mut lib_bytes)?;

												Ok::<bytes::Bytes, crate::Error>(bytes::Bytes::from(lib_bytes))
											}).await??;

											local_libs.insert(lib.name.clone(), lib_bytes);
										}
									}

									let path = profile.path.clone();
									let version = profile.version.clone();

									for entry in profile.data.values_mut() {
										if entry.client.starts_with('/') || entry.server.starts_with('/') {
											macro_rules! read_data {
												($value:expr) => {
													let mut archive_clone = archive.clone();
													let value_clone = $value.clone();
													let lib_bytes = tokio::task::spawn_blocking(move || {
														let mut lib_file = archive_clone.by_name(&value_clone[1..value_clone.len()])?;
														let mut lib_bytes = Vec::new();
														lib_file.read_to_end(&mut lib_bytes)?;

														Ok::<bytes::Bytes, crate::Error>(bytes::Bytes::from(lib_bytes))
													}).await??;

													let split = $value.split('/').last();

													if let Some(last) = split {
														let mut file = last.split('.');

														if let Some(file_name) = file.next() {
															if let Some(ext) = file.next() {
																let path = format!("{}:{}@{}", path.as_deref().unwrap_or(&*format!("net.minecraftforge:forge:{}", version)), file_name, ext);
																$value = format!("[{}]", &path);
																local_libs.insert(path.clone(), bytes::Bytes::from(lib_bytes));

																libs.push(Library {
																	downloads: None,
																	extract: None,
																	name: path,
																	url: Some("".to_string()),
																	natives: None,
																	rules: None,
																	checksums: None,
																	include_in_classpath: false,
																});
															}
														}
													}
												};
											}

											if entry.client.starts_with('/') {
												read_data!(entry.client);
											}

											if entry.server.starts_with('/') {
												read_data!(entry.server);
											}
										}
									}

									let now = Instant::now();
									let libs = futures::future::try_join_all(libs.into_iter().map(|mut lib| async {
										let artifact_path = get_path_from_artifact(&lib.name)?;

										{
											let mut visited_assets = visited_assets.lock().await;
											if visited_assets.contains(&lib.name) {
												if let Some(ref mut downloads) = lib.downloads {
													if let Some(ref mut artifact) = downloads.artifact {
														artifact.url = format_url(&format!("maven/{}", artifact_path));
													}
												} else if lib.url.is_some() {
													lib.url = Some(format_url("maven/"));
												}

												return Ok::<Library, crate::Error>(lib);
											} else {
												visited_assets.push(lib.name.clone())
											}
										}

										let artifact_bytes = if let Some(ref mut downloads) = lib.downloads {
											if let Some(ref mut artifact) = downloads.artifact {
												let res = if artifact.url.is_empty() {
													local_libs.get(&lib.name).cloned()
												} else {
													Some(download_file(
														&artifact.url,
														Some(&*artifact.sha1),
														semaphore.clone(),
													).await?)
												};

												if res.is_some() {
													artifact.url = format_url(&format!("maven/{}", artifact_path));
												}

												res
											} else { None }
										} else if let Some(ref mut url) = lib.url {
											let res = if url.is_empty() {
												local_libs.get(&lib.name).cloned()
											} else {
												Some(download_file(
													url,
													None,
													semaphore.clone(),
												).await?)
											};

											if res.is_some() {
												lib.url = Some(format_url("maven/"));
											}

											res
										} else { None };

										if let Some(bytes) = artifact_bytes {
											upload_file_to_bucket(
												format!("{}/{}", "maven", artifact_path),
												bytes.to_vec(),
												Some("application/java-archive".to_string()),
												uploaded_files_mutex.as_ref(),
												semaphore.clone(),
											).await?;
										}

										Ok::<Library, crate::Error>(lib)
									})).await?;

									let elapsed = now.elapsed();
									info!("elapsed lib dl: {:.2?}", elapsed);

									let new_profile = PartialVersionInfo {
										id: version_info.id,
										inherits_from: version_info.inherits_from,
										release_time: version_info.release_time,
										time: version_info.time,
										main_class: version_info.main_class,
										minecraft_arguments: version_info.minecraft_arguments,
										arguments: version_info.arguments,
										libraries: libs,
										type_: version_info.type_,
										data: Some(profile.data),
										processors: Some(profile.processors),
									};

									let version_path = format!(
										"{}/v{}/versions/{}.json",
										loader_name.as_str(),
										loader_name.as_format(),
										new_profile.id
									);

									upload_file_to_bucket(
										version_path.clone(),
										serde_json::to_vec(&new_profile)?,
										Some("application/json".to_string()),
										uploaded_files_mutex.as_ref(),
										semaphore.clone(),
									).await?;

									return Ok(Some(LoaderVersion {
										id: loader_version_full,
										url: format_url(&version_path),
										stable: false
									}));
								}
							}

							Ok(None)
						}.await
					});

					{
						let len = loaders_futures.len();
						let mut versions = loaders_futures.into_iter().peekable();
						let mut chunk_index = 0;
						while versions.peek().is_some() {
							let now = Instant::now();

							let chunk: Vec<_> = versions.by_ref().take(1).collect();
							let res = futures::future::try_join_all(chunk).await?;
							loaders_versions.extend(res.into_iter().flatten());

							chunk_index += 1;

							let elapsed = now.elapsed();
							info!("loader chunk {}/{len} elapsed: {:.2?}", chunk_index, elapsed);
						}
					}
				}

				versions.lock().await.push(interpulse::api::modded::Version {
					id: minecraft_version,
					stable: true,
					loaders: loaders_versions
				});

				Ok::<(), crate::Error>(())
			});
		}
	}

	{
		let len = version_futures.len();
		let mut versions = version_futures.into_iter().peekable();
		let mut chunk_index = 0;
		while versions.peek().is_some() {
			let now = Instant::now();
			let chunk: Vec<_> = versions.by_ref().take(1).collect();
			futures::future::try_join_all(chunk).await?;

			chunk_index += 1;

			let elapsed = now.elapsed();
			info!("chunk {}/{len} elapsed: {:.2?}", chunk_index, elapsed);
		}
	}

	if let Ok(versions) = Arc::try_unwrap(versions) {
		let mut versions = versions.into_inner();

		versions.sort_by(|x, y| {
			minecraft_versions
				.versions
				.iter()
				.position(|z| match loader_name {
					ForgeLikeLoaders::Forge => x.id.replace("1.7.10_pre4", "1.7.10-pre4") == z.id,
					ForgeLikeLoaders::Neo => x.id == z.id,
				})
				.unwrap_or_default()
				.cmp(
					&minecraft_versions
						.versions
						.iter()
						.position(|z| match loader_name {
							ForgeLikeLoaders::Forge => {
								y.id.replace("1.7.10_pre4", "1.7.10-pre4") == z.id
							}
							ForgeLikeLoaders::Neo => y.id == z.id,
						})
						.unwrap_or_default(),
				)
		});

		for version in &mut versions {
			let loader_versions = maven_metadata.get(&version.id);
			if let Some(loader_versions) = loader_versions {
				version.loaders.sort_by(|x, y| {
					loader_versions
						.iter()
						.position(|z| y.id == z.1)
						.unwrap_or_default()
						.cmp(&loader_versions.iter().position(|z| x.id == z.1).unwrap_or_default())
				});
				version.loaders.reverse();
			}
		}

		upload_file_to_bucket(
			format!("{}/v{}/manifest.json", loader_name.as_str(), loader_name.as_format()),
			serde_json::to_vec(&Manifest { game_versions: versions })?,
			Some("application/json".to_string()),
			uploaded_files_mutex.as_ref(),
			semaphore,
		)
		.await?;
	}

	if let Ok(uploaded_files_mutex) = Arc::try_unwrap(uploaded_files_mutex) {
		uploaded_files.extend(uploaded_files_mutex.into_inner());
	}

	Ok(())
}

const DEFAULT_NEO_MAVEN_METADATA_URL_1: &str =
	"https://maven.neoforged.net/net/neoforged/forge/maven-metadata.xml";
const DEFAULT_NEO_MAVEN_METADATA_URL_2: &str =
	"https://maven.neoforged.net/net/neoforged/neoforge/maven-metadata.xml";
const DEFAULT_FORGE_MAVEN_METADATA_URL: &str =
	"https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";

#[derive(Debug, Deserialize)]
struct Metadata {
	versioning: Versioning,
}

#[derive(Debug, Deserialize)]
struct Versioning {
	versions: Versions,
}

#[derive(Debug, Deserialize)]
struct Versions {
	version: Vec<String>,
}

/// returns ["mc_version": ["array_of": {"original_loader_version", "parsed_loader_version", "is_neoforge"}]]
async fn fetch_forge_metadata(
	url: Option<&str>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<HashMap<String, Vec<(String, String, bool)>>> {
	let metadata: HashMap<String, Vec<String>> = serde_json::from_slice(
		&download_file(url.unwrap_or(DEFAULT_FORGE_MAVEN_METADATA_URL), None, semaphore).await?,
	)?;

	let mut map: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();

	for (mc_version, loader_versions) in metadata {
		let mut versions = Vec::new();

		for loader_version_full in loader_versions {
			let loader_version = loader_version_full.split('-').nth(1);

			if let Some(loader_version_raw) = loader_version {
				let split = loader_version_raw.split('.').collect::<Vec<&str>>();
				let loader_version = if split.len() >= 4 {
					if split[0].parse::<i32>().unwrap_or(0) < 6 {
						format!("{}.{}.{}", split[0], split[1], split[3])
					} else {
						format!("{}.{}.{}", split[1], split[2], split[3])
					}
				} else {
					loader_version_raw.to_string()
				};

				versions.push((loader_version_full, loader_version, false))
			}
		}

		map.insert(mc_version, versions);
	}

	Ok(map)
}

/// returns ["mc_version": ["array_of": {"original_loader_version", "parsed_loader_version", "is_neoforge"}]]
async fn fetch_neo_metadata(
	semaphore: Arc<Semaphore>,
) -> crate::Result<HashMap<String, Vec<(String, String, bool)>>> {
	async fn fetch_values(url: &str, semaphore: Arc<Semaphore>) -> crate::Result<Metadata> {
		Ok(serde_xml_rs::from_str(
			&String::from_utf8(download_file(url, None, semaphore).await?.to_vec())
				.unwrap_or_default(),
		)?)
	}

	let forge_values = fetch_values(DEFAULT_NEO_MAVEN_METADATA_URL_1, semaphore.clone()).await?;
	let neo_values = fetch_values(DEFAULT_NEO_MAVEN_METADATA_URL_2, semaphore).await?;
	let mut map: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();

	for value in forge_values.versioning.versions.version {
		let original = value.clone();
		let parts: Vec<&str> = value.split('-').collect();

		if parts.len() == 2 {
			map.entry(parts[0].to_string()).or_default().push((
				original,
				parts[1].to_string(),
				false,
			));
		}
	}

	for value in neo_values.versioning.versions.version {
		let original = value.clone();
		let mut parts = value.split('.');

		if let Some(major) = parts.next() {
			if let Some(minor) = parts.next() {
				let game_version = format!("1.{}.{}", major, minor);
				map.entry(game_version.clone()).or_default().push((
					original.clone(),
					format!("{}-{}", game_version, original),
					true,
				));
			}
		}
	}

	Ok(map)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ForgeInstallerProfileInstallDataV1 {
	pub mirror_list: String,
	pub target: String,
	/// Path to the Forge universal library
	pub file_path: String,
	pub logo: String,
	pub welcome: String,
	pub version: String,
	/// Maven coordinates of the Forge universal library
	pub path: String,
	pub profile_name: String,
	pub minecraft: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ForgeInstallerProfileManifestV1 {
	pub id: String,
	pub libraries: Vec<Library>,
	pub main_class: Option<String>,
	pub minecraft_arguments: Option<String>,
	pub release_time: DateTime<Utc>,
	pub time: DateTime<Utc>,
	pub type_: VersionType,
	pub assets: Option<String>,
	pub inherits_from: Option<String>,
	pub jar: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ForgeInstallerProfileV1 {
	pub install: ForgeInstallerProfileInstallDataV1,
	pub version_info: ForgeInstallerProfileManifestV1,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ForgeInstallerProfileV2 {
	pub spec: i32,
	pub profile: String,
	pub version: String,
	pub json: String,
	pub path: Option<String>,
	pub minecraft: String,
	pub data: HashMap<String, SidedDataEntry>,
	pub libraries: Vec<Library>,
	pub processors: Vec<Processor>,
}
