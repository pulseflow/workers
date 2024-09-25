use crate::utils::prelude::*;
use interpulse::utils::get_path_from_artifact;

pub struct UploadFile {
	pub file: Bytes,
	pub content_type: Option<String>,
}

pub struct MirrorArtifact {
	pub sha1: Option<String>,
	pub mirrors: DashSet<Mirror>,
}

#[derive(Eq, PartialEq, Hash)]
pub struct Mirror {
	pub path: String,
	pub entire_url: bool,
}

#[tracing::instrument(skip(mirror_artifacts))]
#[allow(clippy::significant_drop_tightening, reason = "clippy is silly")]
pub fn insert_mirrored_artifact(
	artifact: &str,
	sha1: Option<String>,
	mirrors: Vec<String>,
	entire_url: bool,
	mirror_artifacts: &crate::MirrorArtifacts,
) -> crate::utils::Result<()> {
	let val = mirror_artifacts
		.entry(get_path_from_artifact(artifact)?)
		.or_insert(MirrorArtifact {
			sha1,
			mirrors: DashSet::new(),
		});

	for mirror in mirrors {
		val.mirrors.insert(Mirror {
			path: mirror,
			entire_url,
		});
	}

	Ok(())
}

#[must_use]
pub fn check_env_vars() -> bool {
	fn check_var<T: std::str::FromStr>(var: &str) -> bool {
		if dotenvy::var(var)
			.ok()
			.and_then(|s| s.parse::<T>().ok())
			.is_none()
		{
			tracing::warn!(
				"variable `{}` missing in dotenvy or not of type `{}`",
				var,
				std::any::type_name::<T>()
			);
			true
		} else {
			false
		}
	}

	let mut failed = false;
	failed |= check_var::<String>("BASE_URL");
	failed |= check_var::<String>("S3_ACCESS_TOKEN");
	failed |= check_var::<String>("S3_SECRET");
	failed |= check_var::<String>("S3_URL");
	failed |= check_var::<String>("S3_REGION");
	failed |= check_var::<String>("S3_BUCKET_NAME");

	if dotenvy::var("CLOUDFLARE_INTEGRATION")
		.ok()
		.and_then(|x| x.parse::<bool>().ok())
		.unwrap_or(false)
	{
		failed |= check_var::<String>("CLOUDFLARE_TOKEN");
		failed |= check_var::<String>("CLOUDFLARE_ZONE_ID");
	}

	failed
}
