use crate::utils::prelude::*;

pub mod download;
pub mod meta;

/// Default catch-all route handler
pub fn handle_route(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
	Response::ok("")?.with_cors(&CORS_POLICY)
}

/// Handles HEAD requests and fallsback to the backend CDN
pub async fn handle_head(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
	let cdn = ctx.env.var(CDN_BACKEND_URL)?.to_string();
	let url = make_cdn_url(&cdn, get_param(&ctx, "file"))?.to_string();
	Fetch::Request(Request::new(url.as_str(), Method::Head)?).send().await?.with_cors(&CORS_POLICY)
}

/// Handles GET requests and fallbacks to the backend CDN
pub fn handle_method(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
	let cdn = ctx.env.var(CDN_BACKEND_URL)?.to_string();
	let url = make_cdn_url(&cdn, get_param(&ctx, "file"))?;
	console_debug!("falling back to cdn for {url}");
	Response::redirect(url)?.with_cors(&CORS_POLICY)
}
