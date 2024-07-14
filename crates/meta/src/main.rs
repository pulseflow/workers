use crate::utils::{
	check_env_vars, format_url, upload_file_to_bucket, upload_url_to_bucket_mirrors, Error,
	ErrorKind, MirrorArtifact, Result, UploadFile, REQWEST_CLIENT,
};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

mod api;
pub mod utils;

#[tokio::main]
async fn main() -> Result<()> {
	dotenvy::dotenv().ok();

	let subscriber = tracing_subscriber::registry()
		.with(fmt::layer())
		.with(EnvFilter::from_default_env())
		.with(ErrorLayer::default());

	tracing::subscriber::set_global_default(subscriber)?;
	tracing::info!("initialized core metadata tasks. starting!");

	if check_env_vars() {
		tracing::error!(
			"some environment variables are missing! please check your environment and re run"
		);
		return Ok(());
	}

	tracing::info!("starting semaphore");
	let semaphore = Arc::new(Semaphore::new(
		dotenvy::var("CONCURRENCY_LIMIT")
			.ok()
			.and_then(|x| x.parse().ok())
			.unwrap_or(10),
	));

	let upload_files: DashMap<String, UploadFile> = DashMap::new();
	let mirror_artifacts: DashMap<String, MirrorArtifact> = DashMap::new();

	api::minecraft::fetch(semaphore.clone(), &upload_files, &mirror_artifacts).await?;
	api::fabric::fetch_fabric(semaphore.clone(), &upload_files, &mirror_artifacts).await?;
	api::fabric::fetch_quilt(semaphore.clone(), &upload_files, &mirror_artifacts).await?;
	// api::fabric::fetch_legacy_fabric(semaphore.clone(), &upload_files, &mirror_artifacts).await?;
	api::forge::fetch_neo(semaphore.clone(), &upload_files, &mirror_artifacts).await?;
	api::forge::fetch_forge(semaphore.clone(), &upload_files, &mirror_artifacts).await?;

	tracing::info!("uploading metadata files to bucket");
	futures::future::try_join_all(upload_files.iter().map(|x| {
		upload_file_to_bucket(
			x.key().clone(),
			x.value().file.clone(),
			x.value().content_type.clone(),
			&semaphore,
		)
	}))
	.await?;

	tracing::info!("uploading mirrored artifacts to bucket");
	futures::future::try_join_all(mirror_artifacts.iter().map(|x| {
		upload_url_to_bucket_mirrors(
			format!("maven/{}", x.key()),
			x.value()
				.mirrors
				.iter()
				.map(|mirror| {
					if mirror.entire_url {
						mirror.path.clone()
					} else {
						format!("{}{}", mirror.path, x.key())
					}
				})
				.collect(),
			x.sha1.clone(),
			&semaphore,
		)
	}))
	.await?;

	tracing::info!("clearing cloudflare cache on updated artifacts");
	if dotenvy::var("CLOUDFLARE_INTEGRATION")
		.ok()
		.and_then(|x| x.parse::<bool>().ok())
		.unwrap_or(false)
	{
		if let Ok(token) = dotenvy::var("CLOUDFLARE_TOKEN") {
			if let Ok(zone_id) = dotenvy::var("CLOUDFLARE_ZONE_ID") {
				let cache_clears = upload_files
					.into_iter()
					.map(|x| format_url(&x.0))
					.chain(
						mirror_artifacts
							.into_iter()
							.map(|x| format_url(&format!("maven/{}", x.0))),
					)
					.collect::<Vec<_>>();

				tracing::info!("clearing cloudflare chunks");
				for chunk in cache_clears.chunks(500) {
					REQWEST_CLIENT
						.post(format!(
							"https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache"
						))
						.bearer_auth(&token)
						.json(&serde_json::json!({
							"files": chunk
						}))
						.send()
						.await
						.map_err(|err| ErrorKind::Fetch {
							inner: err,
							item: "cloudflare clear cache".to_string(),
						})?
						.error_for_status()
						.map_err(|err| ErrorKind::Fetch {
							inner: err,
							item: "cloudflare clear cache".to_string(),
						})?;
				}
			}
		}
	}

	Ok(())
}
