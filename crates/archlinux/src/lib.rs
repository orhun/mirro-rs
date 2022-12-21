use hyper::{body::Buf, Body, Client, Request, Uri};
use hyper_tls::HttpsConnector;
use tracing::{info, trace};

use crate::response::external::Root;

#[cfg(test)]
mod tests;

mod response;
pub use response::external::Protocol;
pub use response::internal::*;

const ARCHLINUX_MIRRORS: &str = "https://archlinux.org/mirrors/status/json/";
const LOCAL_SOURCE: &str = include_str!("../sample/archlinux.json");

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tracing::instrument()]
pub async fn archlinux() -> Result<ArchLinux> {
    trace!("creating http client");
    let client = Client::builder().build::<_, Body>(HttpsConnector::new());
    let uri = ARCHLINUX_MIRRORS.parse::<Uri>()?;

    trace!("building request");
    let req = Request::builder().uri(uri).body(Body::empty())?;
    let response = client.request(req).await?;

    let bytes = hyper::body::aggregate(response.into_body()).await?;

    let root: Root = serde_json::from_reader(bytes.reader())?;

    let body = ArchLinux::from(root);
    let count = body.countries.len();
    info!("located mirrors from {count} countries");
    Ok(body)
}

pub fn archlinux_fallback() -> Result<ArchLinux> {
    let vals = ArchLinux::from(serde_json::from_str::<Root>(LOCAL_SOURCE)?);
    Ok(vals)
}
