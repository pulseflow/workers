use std::collections::HashMap;

use chrono::{DateTime, Utc};
use interpulse::api::{minecraft::{Library, VersionManifest, VersionType}, modded::{fetch_manifest, Processor, SidedDataEntry}};
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
}

pub async fn retrieve_forge_like_data(
	loader_name: ForgeLikeLoaders,
	current_format_version: usize,
	minecraft_versions: &VersionManifest,
	uploaded_files: &mut Vec<String>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	let maven_metadata = match loader_name {
		ForgeLikeLoaders::Forge => fetch_forge_metadata(None, semaphore.clone()).await?,
		ForgeLikeLoaders::Neo => fetch_neo_metadata(semaphore.clone()).await?
	};
	let old_manifest = fetch_manifest(&format_url(&format!(
		"{}/v{}/manifest.json",
		loader_name.as_str(), current_format_version,
	))).await.ok();

	let old_versions = Arc::new(Mutex::new(if let Some(old_manifest) = old_manifest {
		old_manifest.game_versions
	} else {
		Vec::new()
	}));

	let versions = Arc::new(Mutex::new(Vec::new()));
	let visited_assets_mutex = Arc::new(Mutex::new(Vec::new()));
	let uploaded_files_mutex = Arc::new(Mutex::new(Vec::new()));
	let mut version_futures = Vec::new();

	match loader_name {
		ForgeLikeLoaders::Forge => {
			for
		},
		ForgeLikeLoaders::Neo => {

		}
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

pub async fn fetch_maven_metadata() {

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
