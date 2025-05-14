use crate::utils::prelude::*;
use futures::io::Cursor;
use indexmap::IndexMap;
use interpulse::api::modded::PartialVersionInfo;
use interpulse::utils::get_path_from_artifact;
use itertools::Itertools;

/// # I HATE FORGE I HATE FORGE
#[tracing::instrument(skip(semaphore, upload_files, mirror_artifacts))]
pub async fn fetch_forge(
	semaphore: Arc<Semaphore>,
	upload_files: &DashMap<String, crate::utils::UploadFile>,
	mirror_artifacts: &DashMap<String, crate::utils::MirrorArtifact>,
) -> Result<(), crate::utils::Error> {
	let forge_manifest = crate::utils::fetch_json::<IndexMap<String, Vec<String>>>(
		"https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
		&semaphore,
	)
	.await?;

	let mut format_version = 0;

	// forge versions can be in these formats:
	// 1.9-12.16.0.1886-failtests
	// 1.9-12.16.0-1886
	// 1.9-12.16.0.1880-1.9
	// 1.14.4-28.1.30
	let forge_versions = forge_manifest.into_iter().flat_map(|(game_version, versions)| versions.into_iter().map(|loader_version| {
        // this forge version formats to get the actual forge version. Ex: `1.15.2-31.1.87` -> `31.1.87`
        let version_split = loader_version.split('-').nth(1).unwrap_or(&loader_version).to_string();

        // forge has three installer formats:
        // - format zero (currently unsupported): forge legacy (pre-1.5.2) (binary patch method)
        //   installation: download patch, download minecraft client JAR. combines patch and client JAR and deletes META-INF/.
        //      (pre-1.3-2) client URL: https://maven.minecraftforge.net/net/minecraftforge/forge/{version}/forge-{version}-client.zip
        //      (pre-1.3-2) server URL: https://maven.minecraftforge.net/net/minecraftforge/forge/{version}/forge-{version}-server.zip
        //      (1.3-2-onwards) universal URL: https://maven.minecraftforge.net/net/minecraftforge/forge/{version}/forge-{version}-universal.zip
        // - format one: forge installer legacy (1.5.2~1.12.2~)
        //     installation: extracts install_profile.json from archive. "versionInfo" is the profile's version info. converts it to the modern format
        //     extracts forge library from archive; path is at "install"."path"
        // - format two: forge installer modern (~1.12.2-latest) (processors)
        //     installation: extracts install_profile.json from archive. extracts version.json from archive. combi es the two and extract all libraries
        //     which are embedded into the installer jar.
        //     upload, and leave processors job to the launchers
        if format_version != 1 && &*version_split == "7.8.0.684" {
            format_version = 1;
        } else if format_version != 2 && &*version_split == "14.23.5.2851" {
            format_version = 2;
        }

        ForgeVersion {
            format_version,
            installer_url: format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{loader_version}/forge-{loader_version}-installer.jar"),
            raw: loader_version,
            loader_version: version_split,
            game_version: game_version.clone(),
        }
    })
        .collect::<Vec<_>>())
        .filter(|x| x.format_version != 0) // todo: support format version zero (see above)
        .filter(|x| {
            // these forge versions are broken and cannot be installed
            const BLACKLIST : &[&str] = &[
                "1.12.2-14.23.5.2851", // mismatched `data` field type
                "1.6.1-8.9.0.749", // malformed archive
                "1.6.1-8.9.0.751", // malformed archive
                "1.6.4-9.11.1.960", // malformed archive
                "1.6.4-9.11.1.961", // malformed archive
                "1.6.4-9.11.1.963", // malformed archive
                "1.6.4-9.11.1.964", // malformed archive
            ];

            !BLACKLIST.contains(&&*x.raw)
        })
        .collect::<Vec<_>>();

	fetch(
		interpulse::api::modded::CURRENT_FORGE_FORMAT_VERSION,
		"forge",
		"https://maven.minecraftforge.net/",
		forge_versions,
		semaphore,
		upload_files,
		mirror_artifacts,
	)
	.await
}

#[derive(Debug, Deserialize)]
struct XmlMetadata {
	versioning: XmlVersioning,
}

#[derive(Debug, Deserialize)]
struct XmlVersioning {
	versions: XmlVersions,
}

#[derive(Debug, Deserialize)]
struct XmlVersions {
	version: Vec<String>,
}

#[tracing::instrument(skip(semaphore, upload_files, mirror_artifacts))]
pub async fn fetch_neo(
	semaphore: Arc<Semaphore>,
	upload_files: &DashMap<String, crate::utils::UploadFile>,
	mirror_artifacts: &DashMap<String, crate::utils::MirrorArtifact>,
) -> crate::utils::Result<()> {
	let forge_versions = crate::utils::fetch_xml::<XmlMetadata>(
		"https://maven.neoforged.net/net/neoforged/forge/maven-metadata.xml",
		&semaphore,
	)
	.await?;
	let neo_versions = crate::utils::fetch_xml::<XmlMetadata>(
		"https://maven.neoforged.net/net/neoforged/neoforge/maven-metadata.xml",
		&semaphore,
	)
	.await?;

	let parsed_versions = forge_versions.versioning.versions.version.into_iter().map(|loader_version| {
        // neoforge(forge) versions can be in these formats: `1.20.1-47.1.74` and `47.1.82`
        // this parses them to get the actual Forge version. e.g. `1.20.1-47.1.74` -> `47.1.74`
        let version_split = loader_version.split('-').nth(1).unwrap_or(&loader_version).to_string();

        Ok(ForgeVersion {
            format_version: 2,
            installer_url: format!("https://maven.neoforged.net/net/neoforged/forge/{loader_version}/forge-{loader_version}-installer.jar"),
            raw: loader_version,
            loader_version: version_split,
            game_version: "1.20.1".to_string(), // all neoforge(forge) versions are for 1.20.1
        })
    }).chain(neo_versions.versioning.versions.version.into_iter().map(|loader_version| {
        let mut parts = loader_version.split('.');

        // neoforge(forge) versions are in this format: `20.2.29-beta`, `20.6.119`
        // the first number is the major mc version, the second is the minor mc version, and the third is the neoforge version
        let major = parts.next().ok_or_else(
            || crate::utils::ErrorKind::InvalidInput(format!("unable to find major game version for neoforge {loader_version}"))
        )?;

        let minor = parts.next().ok_or_else(
            || crate::utils::ErrorKind::InvalidInput(format!("Unable to find minor game version for NeoForge {loader_version}"))
        )?;

        let game_version = if minor == "0" {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        };

        Ok(ForgeVersion {
            format_version: 2,
            installer_url: format!("https://maven.neoforged.net/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"),
            loader_version: loader_version.clone(),
            raw: loader_version,
            game_version, // not all neoforge(neo) versions are for 1.20.1
        })
    }))
        .collect::<crate::utils::Result<Vec<_>>>()?
        .into_iter()
        .filter(|x| {
        	const BLACKLIST: &[&str] = &["1.20.1-47.1.7", "47.1.82"]; // nonexistent (404)
        	!BLACKLIST.contains(&&*x.raw)
    	})
		.collect();

	fetch(
		interpulse::api::modded::CURRENT_NEOFORGE_FORMAT_VERSION,
		"neo",
		"https://maven.neoforged.net/",
		parsed_versions,
		semaphore,
		upload_files,
		mirror_artifacts,
	)
	.await
}

#[tracing::instrument(skip(forge_versions, semaphore, upload_files, mirror_artifacts))]
async fn fetch(
	format_version: usize,
	mod_loader: &str,
	maven_url: &str,
	forge_versions: Vec<ForgeVersion>,
	semaphore: Arc<Semaphore>,
	upload_files: &crate::UploadFiles,
	mirror_artifacts: &crate::MirrorArtifacts,
) -> crate::utils::Result<()> {
	tracing::info!("fetching forge mod loader metadata for {mod_loader}!");
	let existing_manifest = crate::utils::fetch_json::<interpulse::api::modded::Manifest>(
		&crate::utils::format_url(&format!("{mod_loader}/v{format_version}/manifest.json",)),
		&semaphore,
	)
	.await
	.ok();

	let fetch_versions = if let Some(existing_manifest) = existing_manifest {
		let mut fetch_versions = Vec::new();

		for version in &forge_versions {
			if !existing_manifest.game_versions.iter().any(|x| {
				x.id == version.game_version
					&& x.loaders.iter().any(|x| x.id == version.loader_version)
			}) {
				fetch_versions.push(version);
			}
		}

		fetch_versions
	} else {
		forge_versions.iter().collect()
	};

	if !fetch_versions.is_empty() {
		let forge_installers = futures::future::try_join_all(
			fetch_versions
				.iter()
				.map(|x| crate::utils::download_file(&x.installer_url, None, &semaphore)),
		)
		.await?;

		#[tracing::instrument(skip(raw, upload_files, mirror_artifacts))]
		async fn read_forge_installer(
			raw: Bytes,
			loader: &ForgeVersion,
			maven_url: &str,
			mod_loader: &str,
			upload_files: &crate::UploadFiles,
			mirror_artifacts: &crate::MirrorArtifacts,
		) -> crate::utils::Result<PartialVersionInfo> {
			type ZipFileReader = async_zip::base::read::seek::ZipFileReader<Cursor<Bytes>>;
			tracing::trace!("reading forge installer for {}", loader.loader_version);

			let cursor = Cursor::new(raw);
			let mut zip = ZipFileReader::new(cursor).await?;

			#[tracing::instrument(skip(zip))]
			async fn read_file(
				zip: &mut ZipFileReader,
				file_name: &str,
			) -> crate::utils::Result<Option<Vec<u8>>> {
				let zip_index_option = zip
					.file()
					.entries()
					.iter()
					.position(|f| f.filename().as_str().unwrap_or_default() == file_name);

				if let Some(zip_index) = zip_index_option {
					let mut buffer = Vec::new();
					let mut reader = zip.reader_with_entry(zip_index).await?;
					reader.read_to_end_checked(&mut buffer).await?;

					Ok(Some(buffer))
				} else {
					Ok(None)
				}
			}

			#[tracing::instrument(skip(zip))]
			async fn read_json<T: DeserializeOwned>(
				zip: &mut ZipFileReader,
				file_name: &str,
			) -> crate::utils::Result<Option<T>> {
				if let Some(file) = read_file(zip, file_name).await? {
					Ok(Some(serde_json::from_slice(&file)?))
				} else {
					Ok(None)
				}
			}

			if loader.format_version == 1 {
				#[derive(Deserialize, Debug)]
				#[serde(rename_all = "camelCase")]
				struct ForgeInstallerProfileInstallDataV1 {
					// pub mirror_list: String,
					// pub target: String,
					/// Path to the Forge universal library
					pub file_path: String,
					// pub logo: String,
					// pub welcome: String,
					// pub version: String,
					/// Maven coordinates of the Forge universal library
					pub path: String,
					// pub profile_name: String,
					pub minecraft: String,
				}

				#[derive(Deserialize, Debug)]
				#[serde(rename_all = "camelCase")]
				struct ForgeInstallerProfileManifestV1 {
					pub id: String,
					pub libraries: Vec<interpulse::api::minecraft::Library>,
					pub main_class: Option<String>,
					pub minecraft_arguments: Option<String>,
					pub release_time: DateTime<Utc>,
					pub time: DateTime<Utc>,
					pub type_: interpulse::api::minecraft::VersionType,
					// pub assets: Option<String>,
					// pub inherits_from: Option<String>,
					// pub jar: Option<String>,
				}

				#[derive(Deserialize, Debug)]
				#[serde(rename_all = "camelCase")]
				struct ForgeInstallerProfileV1 {
					pub install: ForgeInstallerProfileInstallDataV1,
					pub version_info: ForgeInstallerProfileManifestV1,
				}

				let install_profile =
					read_json::<ForgeInstallerProfileV1>(&mut zip, "install_profile.json")
						.await?
						.ok_or_else(|| {
							crate::utils::ErrorKind::InvalidInput(format!(
								"No install_profile.json present for loader {}",
								loader.installer_url
							))
						})?;

				let forge_library = read_file(&mut zip, &install_profile.install.file_path)
					.await?
					.ok_or_else(|| {
						crate::utils::ErrorKind::InvalidInput(format!(
							"No forge library present for loader {}",
							loader.installer_url
						))
					})?;

				upload_files.insert(
					format!(
						"maven/{}",
						get_path_from_artifact(&install_profile.install.path)?
					),
					crate::utils::UploadFile {
						file: Bytes::from(forge_library),
						content_type: None,
					},
				);

				Ok(PartialVersionInfo {
					id: install_profile.version_info.id,
					inherits_from: install_profile.install.minecraft,
					release_time: install_profile.version_info.release_time,
					time: install_profile.version_info.time,
					main_class: install_profile.version_info.main_class,
					minecraft_arguments: install_profile.version_info.minecraft_arguments.clone(),
					arguments: install_profile.version_info.minecraft_arguments.map(|x| {
						std::iter::once(&(
							interpulse::api::minecraft::ArgumentType::Game,
							x.split(' ')
								.map(|x| {
									interpulse::api::minecraft::Argument::Normal(x.to_string())
								})
								.collect(),
						))
						.cloned()
						.collect()
					}),
					libraries: install_profile
						.version_info
						.libraries
						.into_iter()
						.map(|mut lib| {
							// any non-forge libraries extracted are mirrored from other mavens,
							// unlesss the url doesn't exist or it is available on mojang's servers
							if let Some(ref url) = lib.url {
								if lib.name == install_profile.install.path {
									lib.url = Some(crate::utils::format_url("maven/"));
								} else if !url.is_empty()
									&& !url.contains("https://libraries.minecraft.net/")
								{
									crate::utils::insert_mirrored_artifact(
										&lib.name,
										None,
										vec![
											url.clone(),
											"https://maven.creeperhost.net/".to_string(),
											maven_url.to_string(),
										],
										false,
										mirror_artifacts,
									)?;

									lib.url = Some(crate::utils::format_url("maven/"));
								}
							}

							Ok(lib)
						})
						.collect::<crate::utils::Result<Vec<_>>>()?,
					type_: install_profile.version_info.type_,
					data: None,
					processors: None,
				})
			} else if loader.format_version == 2 {
				#[derive(Deserialize, Debug)]
				#[serde(rename_all = "camelCase")]
				struct ForgeInstallerProfileV2 {
					// pub spec: i32,
					// pub profile: String,
					// pub version: String,
					// pub json: String,
					// pub path: Option<String>,
					// pub minecraft: String,
					pub data: HashMap<String, interpulse::api::modded::SidedDataEntry>,
					pub libraries: Vec<interpulse::api::minecraft::Library>,
					pub processors: Vec<interpulse::api::modded::Processor>,
				}

				async fn mirror_forge_library(
					mut zip: ZipFileReader,
					mut lib: interpulse::api::minecraft::Library,
					maven_url: &str,
					upload_files: &crate::UploadFiles,
					mirror_artifacts: &crate::MirrorArtifacts,
				) -> crate::utils::Result<interpulse::api::minecraft::Library> {
					let artifact_path = get_path_from_artifact(&lib.name)?;

					if let Some(ref mut artifact) =
						lib.downloads.as_mut().and_then(|x| x.artifact.as_mut())
					{
						if !artifact.url.is_empty() {
							crate::utils::insert_mirrored_artifact(
								&lib.name,
								Some(artifact.sha1.clone()),
								vec![artifact.url.clone()],
								true,
								mirror_artifacts,
							)?;

							artifact.url =
								crate::utils::format_url(&format!("maven/{artifact_path}"));

							return Ok(lib);
						}
					} else if let Some(url) = &lib.url {
						if !url.is_empty() {
							crate::utils::insert_mirrored_artifact(
								&lib.name,
								None,
								vec![
									url.clone(),
									"https://libraries.minecraft.net/".to_string(),
									"https://maven.creeperhost.net/".to_string(),
									maven_url.to_string(),
								],
								false,
								mirror_artifacts,
							)?;

							lib.url = Some(crate::utils::format_url("maven/"));

							return Ok(lib);
						}
					}

					// other libraries are generally available in the `maven/` directory
					// of the forge installer. if they don't exist yet, they are generated by a processor.
					let extract_path = format!("maven/{artifact_path}");
					if let Some(file) = read_file(&mut zip, &extract_path).await? {
						upload_files.insert(
							extract_path,
							crate::utils::UploadFile {
								file: Bytes::from(file),
								content_type: None,
							},
						);

						lib.url = Some(crate::utils::format_url("maven/"));
					} else {
						lib.downloadable = false;
					}

					Ok(lib)
				}

				let install_profile =
					read_json::<ForgeInstallerProfileV2>(&mut zip, "install_profile.json")
						.await?
						.ok_or_else(|| {
							crate::utils::ErrorKind::InvalidInput(format!(
								"No install_profile.json present for loader {}",
								loader.installer_url
							))
						})?;

				let mut version_info = read_json::<PartialVersionInfo>(&mut zip, "version.json")
					.await?
					.ok_or_else(|| {
						crate::utils::ErrorKind::InvalidInput(format!(
							"No version.json present for loader {}",
							loader.installer_url
						))
					})?;

				version_info.processors = Some(install_profile.processors);
				version_info
					.libraries
					.extend(install_profile.libraries.into_iter().map(|mut x| {
						x.include_in_classpath = false;

						x
					}));

				version_info.libraries =
					futures::future::try_join_all(version_info.libraries.into_iter().map(|lib| {
						mirror_forge_library(
							zip.clone(),
							lib,
							maven_url,
							upload_files,
							mirror_artifacts,
						)
					}))
					.await?;

				// in forge modern installers, processors are run during the install process. some processors
				// are extracted from the installer jar. this function finds these files, extracts them, and uploads them
				// and registers them as libraries instead.
				// for example:
				// "BINPATCH": {
				//      "client": "/data/client.lzma",
				//      "server": "/data/server.lzma"
				//     },
				// becomes:
				// "BINPATCH": {
				//      "client": "[net.minecraftforge:forge:1.20.3-49.0.1:shim:client@lzma]",
				//      "server": "[net.minecraftforge:forge:1.20.3-49.0.1:shim:server@lzma]"
				// },
				// the resulting library is added to the profile's libraries
				let mut new_data = HashMap::new();
				for (key, entry) in install_profile.data {
					async fn extract_data(
						zip: &mut ZipFileReader,
						key: &str,
						value: &str,
						upload_files: &crate::UploadFiles,
						libs: &mut Vec<interpulse::api::minecraft::Library>,
						mod_loader: &str,
						version: &ForgeVersion,
					) -> crate::utils::Result<String> {
						let extract_file = read_file(zip, &value[1..value.len()])
							.await?
							.ok_or_else(|| {
								crate::utils::ErrorKind::InvalidInput(format!(
									"Unable reading data key {key} at path {value}",
								))
							})?;

						let file_name = value.split('/').next_back().ok_or_else(|| {
							crate::utils::ErrorKind::InvalidInput(format!(
								"Unable reading filename for data key {key} at path {value}",
							))
						})?;

						let mut file = file_name.split('.');
						let file_name = file.next().ok_or_else(|| {
							crate::utils::ErrorKind::InvalidInput(format!(
								"Unable reading filename only for data key {key} at path {value}",
							))
						})?;
						let ext = file.next().ok_or_else(|| {
							crate::utils::ErrorKind::InvalidInput(format!(
								"Unable reading extension only for data key {key} at path {value}",
							))
						})?;

						let path = format!(
							"org.polyfrost.meta:{}-installer-extracts:{}:{}@{}",
							mod_loader, version.raw, file_name, ext
						);

						upload_files.insert(
							format!("maven/{}", get_path_from_artifact(&path)?),
							crate::utils::UploadFile {
								file: Bytes::from(extract_file),
								content_type: None,
							},
						);

						libs.push(interpulse::api::minecraft::Library {
							downloads: None,
							extract: None,
							name: path.clone(),
							url: Some(crate::utils::format_url("maven/")),
							natives: None,
							rules: None,
							checksums: None,
							include_in_classpath: false,
							downloadable: true,
						});

						Ok(format!("[{path}]"))
					}

					let client = if entry.client.starts_with('/') {
						extract_data(
							&mut zip,
							&key,
							&entry.client,
							upload_files,
							&mut version_info.libraries,
							mod_loader,
							loader,
						)
						.await?
					} else {
						entry.client.clone()
					};

					let server = if entry.server.starts_with('/') {
						extract_data(
							&mut zip,
							&key,
							&entry.server,
							upload_files,
							&mut version_info.libraries,
							mod_loader,
							loader,
						)
						.await?
					} else {
						entry.server.clone()
					};

					new_data.insert(
						key.clone(),
						interpulse::api::modded::SidedDataEntry { client, server },
					);
				}

				version_info.data = Some(new_data);

				Ok(version_info)
			} else {
				Err(crate::utils::ErrorKind::InvalidInput(format!(
					"Unknown format version {} for loader {}",
					loader.format_version, loader.installer_url
				))
				.into())
			}
		}

		let forge_version_infos =
			futures::future::try_join_all(forge_installers.into_iter().enumerate().map(
				|(index, raw)| {
					let loader = fetch_versions[index];

					read_forge_installer(
						raw,
						loader,
						maven_url,
						mod_loader,
						upload_files,
						mirror_artifacts,
					)
				},
			))
			.await?;

		let serialized_version_manifests = forge_version_infos
			.iter()
			.map(|x| serde_json::to_vec(x).map(Bytes::from))
			.collect::<Result<Vec<_>, serde_json::Error>>()?;

		serialized_version_manifests
			.into_iter()
			.enumerate()
			.for_each(|(index, bytes)| {
				let loader = fetch_versions[index];

				let version_path = format!(
					"{mod_loader}/v{format_version}/versions/{}.json",
					loader.loader_version
				);

				upload_files.insert(
					version_path,
					crate::utils::UploadFile {
						file: bytes,
						content_type: Some("application/json".to_string()),
					},
				);
			});

		let forge_manifest_path = format!("{mod_loader}/v{format_version}/manifest.json",);

		let manifest = interpulse::api::modded::Manifest {
			game_versions: forge_versions
				.into_iter()
				.sorted_by(|a, b| b.game_version.cmp(&a.game_version))
				.rev()
				.chunk_by(|x| x.game_version.clone())
				.into_iter()
				.map(|(game_version, loaders)| interpulse::api::modded::Version {
					id: game_version,
					stable: true,
					loaders: loaders
						.map(|x| interpulse::api::modded::LoaderVersion {
							url: crate::utils::format_url(&format!(
								"{mod_loader}/v{format_version}/versions/{}.json",
								x.loader_version
							)),
							id: x.loader_version,
							stable: false,
						})
						.collect(),
				})
				.collect(),
		};

		upload_files.insert(
			forge_manifest_path,
			crate::utils::UploadFile {
				file: Bytes::from(serde_json::to_vec(&manifest)?),
				content_type: Some("application/json".to_string()),
			},
		);
	}

	Ok(())
}

#[derive(Debug)]
struct ForgeVersion {
	pub format_version: usize,
	pub raw: String,
	pub loader_version: String,
	pub game_version: String,
	pub installer_url: String,
}
