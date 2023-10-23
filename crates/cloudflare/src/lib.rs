use crate::utils::prelude::*;

mod meta;
mod routes;
mod utils;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
	log_request(&req);
	utils::set_panic_hook();

	Router::new()
		.get_async("/data/:hash/versions/:version/:file", routes::download::handle_version_download)
		.options("/*route", routes::handle_route)
		.head_async("/*file", routes::handle_head)
		.or_else_any_method("/*file", routes::handle_method)
		.run(req, env)
		.await
}
