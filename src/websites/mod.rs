use async_trait::async_trait;
use futures::FutureExt;
use isahc::prelude::*;
use tokio_util::compat::FuturesAsyncReadCompatExt;
pub mod kkgal;
pub mod liuli;
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
    async fn download_http_game(
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
        let mut last_byte_count = 0;
        let mut last_timestamp = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let progress = metrics.download_progress().0 as f64 / metrics.download_progress().1 as f64;
            let speed = (metrics.download_progress().0 - last_byte_count) as f64 / last_timestamp.elapsed().as_secs_f64();
            last_byte_count = metrics.download_progress().0;
            last_timestamp = std::time::Instant::now();
            log_client.log(
                crate::log::LoggingLevel::StatusReport,
                &format!(
                    "{} => {} {}({:.2}%) {}/s",
                    file,
                    humantime::format_duration(std::time::Duration::from_secs(metrics.total_time().as_secs())),
                    byte_unit::Byte::from_bytes(metrics.download_progress().0 as u128)
                        .get_appropriate_unit(true)
                        .to_string(),
                    progress,
                    byte_unit::Byte::from_bytes(speed as u128)
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
