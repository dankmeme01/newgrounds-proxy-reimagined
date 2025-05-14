use moka::future::Cache;
use rocket::{data::ToByteUnit, get, launch, routes};
use std::{path::PathBuf, sync::Arc};

mod logger;
use logger::*;

struct State {
    cache: Cache<String, Arc<[u8]>>,
    client: reqwest::Client,
}

impl State {
    pub fn new() -> Self {
        Self {
            cache: Cache::new(128),

            client: reqwest::ClientBuilder::new()
                .user_agent("ng-proxy/v1.0.0")
                .build()
                .expect("Failed to create reqwest client"),
        }
    }

    pub async fn get_by_url(&self, url: &str) -> Option<Arc<[u8]>> {
        self.cache.get(url).await
    }

    pub async fn create_with_data(&self, url: String, data: Vec<u8>) -> std::io::Result<Arc<[u8]>> {
        let value = Arc::<[u8]>::from(data.into_boxed_slice());

        self.cache.insert(url, value.clone()).await;

        Ok(value)
    }

    pub async fn download_and_cache(&self, url: String) -> Result<Arc<[u8]>, String> {
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let resp = resp.error_for_status().map_err(|s| s.to_string())?;

        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        let data = bytes.to_vec();

        if data.len() > 32.megabytes() {
            warn!(
                "Downloaded big file ({} bytes), it will not be cached in RAM!",
                data.len()
            );

            return Ok(Arc::<[u8]>::from(data.into_boxed_slice()));
        } else {
            info!("Downloaded {}, saving to cache", filename_from_url(&url));
        }

        self.create_with_data(url, data)
            .await
            .map_err(|e| e.to_string())
    }
}

fn filename_from_url(url: &str) -> &str {
    let url = url.trim_end_matches('/');

    let last_slash = url.rfind('/');

    match last_slash {
        Some(n) => &url[n + 1..],
        None => url,
    }
}

#[get("/<path..>")]
async fn forwarder(path: PathBuf, state: &rocket::State<State>) -> Result<Arc<[u8]>, String> {
    let full_url = format!("https://audio.ngfiles.com/{}", path.display());

    if let Some(file) = state.get_by_url(&full_url).await {
        info!("Cache hit for {}", filename_from_url(&full_url));
        Ok(file)
    } else {
        info!(
            "Cache miss for {}, downloading",
            filename_from_url(&full_url)
        );
        Ok(state.download_and_cache(full_url).await?)
    }
}

#[launch]
fn rocket() -> _ {
    log::set_logger(Logger::instance()).expect("Failed to set logger");
    log::set_max_level(LogLevelFilter::Info);

    info!(
        "Running ngproxy server version {}",
        env!("CARGO_PKG_VERSION")
    );

    rocket::build()
        .mount("/", routes![forwarder])
        .manage(State::new())
}
