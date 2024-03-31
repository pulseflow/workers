use log::error;
use meta_workers::api;
use meta_workers::utils::*;

#[tokio::main]
async fn main() {
	env_logger::init();

	if check_env_vars() {
		error!("some environment variables are missing!");
		return;
	}

	let mut timer = tokio::time::interval(Duration::from_secs(60 * 60));
	let semaphore = Arc::new(Semaphore::new(10));

	loop {
		timer.tick().await;

		let mut uploded_files = Vec::new();

		let versions =
			match api::minecraft::retrieve_data(&mut uploded_files, semaphore.clone()).await {
				Ok(res) => Some(res),
				Err(err) => {
					error!("{:?}", err);

					None
				}
			};

		if let Some(manifest) = versions {
			match api::fabric::retrieve_data(&manifest, &mut uploded_files, semaphore.clone()).await
			{
				Ok(..) => {}
				Err(err) => error!("{:?}", err),
			};

			match api::forge::retrieve_data(&manifest, &mut uploded_files, semaphore.clone()).await
			{
				Ok(..) => {}
				Err(err) => error!("{:?}", err),
			};

			match api::quilt::retrieve_data(&manifest, &mut uploded_files, semaphore.clone()).await
			{
				Ok(..) => {}
				Err(err) => error!("{:?}", err),
			};

			match api::legacy_fabric::retrieve_data(
				&manifest,
				&mut uploaded_files,
				semaphore.clone(),
			)
			.await
			{
				Ok(..) => {}
				Err(err) => error!("{:?}", err),
			};

			match api::neo::retrieve_data(&manifest, &mut uploded_files, semaphore.clone()).await {
				Ok(..) => {}
				Err(err) => error!("{:?}", err),
			};
		}
	}
}
