use async_trait::async_trait;
use futures::io::AsyncReadExt;
use futures::FutureExt;
use isahc::prelude::*;
use tokio::io::AsyncWriteExt;
use tokio_util::compat::FuturesAsyncReadCompatExt;
pub mod kkgal;
#[async_trait]
pub trait GalgameWebsite {
    async fn fetch_metadata(
        &self,
        page: u32,
        overwrite: bool,
        database: &crate::saved::GameTextDatabase,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Vec<crate::saved::GameTextInformation>, String>;
    async fn download_user_avatars(
        &self,
        avatar_url: String,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Vec<u8>, String>;
    async fn download_screenshot(
        &self,
        screenshot_url: String,
        game_info: &crate::saved::GameTextInformation,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Vec<u8>, String>;
    async fn download_game(
        &self,
        link: String,
        game_info: &crate::saved::GameTextInformation,
        file: String,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<(), String>;
}
async fn game_download_helper<'a>(
    response: isahc::ResponseFuture<'a>,
    file: &str,
    log_client: &crate::log::LoggingClient,
) -> Result<(), String> {
    let mut response_mapped = response.await.map_err(|x| x.to_string())?;
    let metrics = response_mapped.metrics().unwrap().clone();
    let mut stream = response_mapped.body_mut().compat();
    let mut file_handle = tokio::fs::File::create(file)
        .await
        .map_err(|x| x.to_string())?;
    async fn logger(log_client: &crate::log::LoggingClient, metrics: &isahc::Metrics, file: &str) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            log_client.log(
                crate::log::LoggingLevel::StatusReport,
                &format!(
                    "{} {} {} {}/s",
                    file,
                    metrics.total_time().as_secs(),
                    byte_unit::Byte::from_bytes(metrics.download_progress().0 as u128)
                        .get_appropriate_unit(true)
                        .to_string(),
                    byte_unit::Byte::from_bytes(metrics.download_speed() as u128)
                        .get_appropriate_unit(true)
                        .to_string()
                ),
            )
        }
    }
    futures::select! {
        result = tokio::io::copy(&mut stream, &mut file_handle).fuse() => return result.map_err(|x| x.to_string()).map(|_| ()),
        _ = logger(log_client, &metrics, file).fuse() => Err(String::from("Error while waiting logger to finish")),
    }
}
