pub use crate::Error;
pub use log::{
	error,
	info,
	warn,
};
pub use serde::{
	Deserialize,
	Serialize,
};
pub use std::{
	sync::Arc,
	time::{
		Duration,
		Instant,
	},
};
pub use tokio::sync::{
	Mutex,
	Semaphore,
};

/// Download a file via Semaphore
pub async fn download_file(
	url: &str,
	sha1: Option<&str>,
	semaphore: Arc<Semaphore>,
) -> Result<bytes::Bytes, Error> {
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
) -> Result<bytes::Bytes, Error> {
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
