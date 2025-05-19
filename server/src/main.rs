use moka::future::Cache;
use rocket::{
    Request, Response,
    data::ToByteUnit,
    get,
    http::{ContentType, Status},
    launch,
    response::Responder,
    routes,
};
use std::{io::Cursor, ops::Deref, path::PathBuf, sync::Arc};

mod logger;
use logger::*;

#[derive(Clone, Debug)]
struct SongArc(pub Arc<[u8]>);

impl Deref for SongArc {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl AsRef<[u8]> for SongArc {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

struct State {
    cache: Cache<String, SongArc>,
    client: reqwest::Client,
}

impl State {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder().max_capacity(128).build(),

            client: reqwest::ClientBuilder::new()
                .user_agent("ng-proxy/v1.0.0")
                .build()
                .expect("Failed to create reqwest client"),
        }
    }

    pub async fn get_by_url(&self, url: &str) -> Option<SongArc> {
        self.cache.get(url).await
    }

    pub async fn create_with_data(&self, url: String, data: Vec<u8>) -> std::io::Result<SongArc> {
        let value = SongArc(Arc::<[u8]>::from(data.into_boxed_slice()));

        self.cache.insert(url, value.clone()).await;

        Ok(value)
    }

    pub async fn download_and_cache(&self, url: String) -> Result<SongArc, String> {
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

            return Ok(SongArc(Arc::<[u8]>::from(data.into_boxed_slice())));
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

struct SongResponse(SongArc);

impl SongResponse {
    pub fn new(data: SongArc) -> Self {
        Self(data)
    }
}

#[rocket::async_trait]
impl<'r> Responder<'r, 'static> for SongResponse {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .header(ContentType::MP3)
            .sized_body(self.0.len(), Cursor::new(self.0))
            .ok()
    }
}

#[get("/")]
async fn index() -> Status {
    Status::NotFound
}

#[get("/favicon.ico")]
async fn favicon() -> Status {
    Status::NotFound
}

#[get("/<path..>")]
async fn forwarder(path: PathBuf, state: &rocket::State<State>) -> Result<SongResponse, Status> {
    let full_url = format!("https://audio.ngfiles.com/{}", path.display());
    let filename = filename_from_url(&full_url);

    if let Some(file) = state.get_by_url(&full_url).await {
        info!("Cache hit for {}", filename);
        Ok(SongResponse::new(file))
    } else {
        info!("Cache miss for {}, downloading", filename);
        let filename = filename.to_owned();

        let file = match state.download_and_cache(full_url).await {
            Ok(x) => x,
            Err(err) => {
                warn!("Error fetching filename '{filename}': {err}");
                return Err(Status::InternalServerError);
            }
        };

        Ok(SongResponse::new(file))
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
        .mount("/", routes![index, favicon, forwarder])
        .manage(State::new())
}
