pub use log::{error, info, warn};
pub use serde::{Deserialize, Serialize};
pub use std::{
	sync::Arc,
	time::{Duration, Instant},
};
pub use tokio::sync::{Mutex, RwLock, Semaphore};

pub mod fabric_based;
pub mod forge_based;

use s3::creds::Credentials;
use s3::Bucket;
use s3::Region;

lazy_static::lazy_static! {
	static ref CLIENT : Bucket = {
		let region = dotenvy::var("S3_REGION").unwrap();
		let b = Bucket::new(
			&dotenvy::var("S3_BUCKET_NAME").unwrap(),
			if &*region == "r2" {
				Region::R2 { account_id: dotenvy::var("S3_URL").unwrap() }
			} else {
				Region::Custom { region: region.clone(), endpoint: dotenvy::var("S3_URL").unwrap() }
			},
			Credentials::new(
				Some(&*dotenvy::var("S3_ACCESS_TOKEN").unwrap()),
				Some(&*dotenvy::var("S3_SECRET").unwrap()),
				None, None, None,
			).unwrap(),
		).unwrap();

		if region == "path-style" { b.with_path_style() } else { b }
	};
}

pub async fn upload_file_to_bucket(
	path: String,
	bytes: Vec<u8>,
	content_type: Option<String>,
	uploaded_files: &Mutex<Vec<String>>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<()> {
	let _permit = semaphore.acquire().await?;
	info!("{} started uploading", path);
	let key = path.clone();

	for attempt in 1..=4 {
		let result = if let Some(ref content_type) = content_type {
			CLIENT.put_object_with_content_type(key.clone(), &bytes, content_type).await
		} else {
			CLIENT.put_object(key.clone(), &bytes).await
		}
		.map_err(|err| crate::Error::S3Error { inner: err, file: path.clone() });

		match result {
			Ok(_) => {
				{
					info!("{} done uploading", path);
					let mut uploaded_files = uploaded_files.lock().await;
					uploaded_files.push(key);
				}

				return Ok(());
			}
			Err(_) if attempt <= 3 => continue,
			Err(_) => {
				result?;
			}
		}
	}

	unreachable!()
}

/// Download a file via Semaphore
pub async fn download_file(
	url: &str,
	sha1: Option<&str>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<bytes::Bytes> {
	let _permit = semaphore.acquire().await?;
	info!("{} started downloading", url);
	let val = interpulse::utils::download_file(url, sha1).await?;
	info!("{} finished downloading", url);
	Ok(val)
}

/// Download file mirrors via Semaphore
pub async fn download_file_mirrors(
	base: &str,
	mirrors: &[&str],
	sha1: Option<&str>,
	semaphore: Arc<Semaphore>,
) -> crate::Result<bytes::Bytes> {
	let _permit = semaphore.acquire().await?;
	info!("{} started downloading", base);
	let val = interpulse::utils::download_file_mirrors(base, mirrors, sha1).await?;
	info!("{} finished downloading", base);
	Ok(val)
}

/// Format a URL Path to use BASE_URL
pub fn format_url(path: &str) -> String {
	format!("{}/{}", &*dotenvy::var("BASE_URL").unwrap(), path)
}

/// Check for missing ENV variables
pub fn check_env_vars() -> bool {
	let mut failed = false;

	failed |= check_var::<String>("BASE_URL");
	failed |= check_var::<String>("S3_ACCESS_TOKEN");
	failed |= check_var::<String>("S3_SECRET");
	failed |= check_var::<String>("S3_URL");
	failed |= check_var::<String>("S3_REGION");
	failed |= check_var::<String>("S3_BUCKET_NAME");

	failed
}

fn check_var<T: std::str::FromStr>(var: &str) -> bool {
	if dotenvy::var(var).ok().and_then(|s| s.parse::<T>().ok()).is_none() {
		warn!(
			"variable `{}` missing in dotenvy or not of type `{}`",
			var,
			std::any::type_name::<T>()
		);

		true
	} else {
		false
	}
}
