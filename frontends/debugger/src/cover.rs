use std::sync::Arc;

use futures::AsyncReadExt as _;
use futures::future::BoxFuture;
use gpui::http_client::http::HeaderValue;
use gpui::http_client::{AsyncBody, HttpClient, Request, Response, Result, Url, http};

#[derive(Clone)]
pub(crate) struct ArtQuery {
    pub game: core_model::GameId,
    pub region: Option<core_model::Region>,
    pub fallback: Option<Arc<str>>,
}

pub(crate) async fn resolve(client: Arc<dyn HttpClient>, query: &ArtQuery) -> Option<Arc<str>> {
    match fetch_sly_cover(client, query).await {
        Ok(url) => Some(url.into()),
        Err(err) => {
            tracing::debug!("artwork database unavailable ({err}), using fallback");
            query.fallback.clone()
        }
    }
}

async fn fetch_sly_cover(client: Arc<dyn HttpClient>, query: &ArtQuery) -> Result<String> {
    let url = artwork::layle::metadata_url(&query.game);
    let mut response = client.get(&url, AsyncBody::empty(), true).await?;
    let mut body = Vec::new();
    response.body_mut().read_to_end(&mut body).await?;
    if !response.status().is_success() {
        return Err(gpui::http_client::anyhow!("status {}", response.status()));
    }

    let metadata = artwork::layle::parse_metadata(&body)?;
    let art = artwork::layle::select_art(&metadata.art, "box-front", query.region.as_slice())
        .ok_or_else(|| gpui::http_client::anyhow!("no box front art for {}", query.game.sha1))?;
    Ok(artwork::layle::art_url(&query.game, &art))
}

pub(crate) struct ArtAuthClient(pub Arc<dyn HttpClient>);

impl HttpClient for ArtAuthClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        self.0.user_agent()
    }

    fn proxy(&self) -> Option<&Url> {
        self.0.proxy()
    }

    fn send(&self, mut req: Request<AsyncBody>) -> BoxFuture<'static, Result<Response<AsyncBody>>> {
        if artwork::layle::is_art_url(&req.uri().to_string()) {
            req.extensions_mut()
                .insert(gpui::http_client::RedirectPolicy::NoFollow);
            req.headers_mut().remove(http::header::AUTHORIZATION);

            if let Some(auth) = settings::authorization()
                && let Ok(mut value) = HeaderValue::from_str(&auth)
            {
                value.set_sensitive(true);
                req.headers_mut().insert(http::header::AUTHORIZATION, value);
            }
        }

        self.0.send(req)
    }
}
