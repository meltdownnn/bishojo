use async_trait::async_trait;
use futures::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
pub mod kkgal;
#[async_trait]
pub trait GalgameWebsite {
    async fn fetch_metadata(
        &self,
        page: u32,
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
        predicted_size: Option<u128>,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
        cache_size: usize,
        // TODO: add direct link support
    ) -> Result<(), String>;
}
async fn game_download_helper<'a>(
    response: isahc::ResponseFuture<'a>,
    file: &str,
    log_client: &crate::log::LoggingClient,
    cache_size: usize,
    predicted_size: Option<u128>
) -> Result<(), String> {
    let mut response = response.await.map_err(|x| x.to_string())?;
    let stream = response.body_mut();
    let mut file_handle = tokio::fs::File::create(file)
        .await
        .map_err(|x| x.to_string())?;
    let file = &file[file.rfind('/').unwrap_or(0)..];
    let mut file_size = 0;
    //futures::io::copy(stream, &mut file_handle.compat_write()).await.map_err(|x| x.to_string()).map(|_| ())
    let mut cache_vector = Vec::with_capacity(cache_size);
    let mut last_instant = std::time::Instant::now();
    let first_instant =
        time::OffsetDateTime::try_now_local().unwrap_or(time::OffsetDateTime::now_utc());
    let mut last_len = 0;
    log_client.log(
        crate::log::LoggingLevel::Message,
        &format!(
            "Download for {} started at {}",
            file,
            first_instant.to_string()
        ),
    );
    loop {
        let output_len = stream
            .read(&mut cache_vector)
            .await
            .map_err(|x| x.to_string())?;
        file_handle
            .write_all(&cache_vector)
            .await
            .map_err(|x| x.to_string())?;
        file_size += output_len;
        let file_size_appropriate =
            byte_unit::Byte::from_bytes(file_size as u128).get_appropriate_unit(true);
        if output_len == 0 {
            file_handle.sync_all().await.map_err(|x| x.to_string())?;
            log_client.log(
                crate::log::LoggingLevel::Message,
                &format!(
                    "{} downloaded - {}",
                    file,
                    file_size_appropriate.to_string()
                ),
            );
            return Ok(());
        }
        if last_instant.elapsed() > std::time::Duration::from_secs(2) {
            let speed = (file_size - last_len) as f64 / last_instant.elapsed().as_secs_f64();
            let speed = byte_unit::Byte::from_bytes(speed as u128).get_appropriate_unit(true);
            log_client.log(
                crate::log::LoggingLevel::StatusReport,
                &format!(
                    "{} {} {}| {}/s",
                    file,
                    file_size_appropriate.to_string(),
                    if let Some(i) = predicted_size {
                        let precentage = (file_size as f64 / i as f64 * 100 as f64) as u8;
                        format!("{}% ", precentage)
                    } else {
                        String::new()
                    },
                    speed.to_string()
                ),
            );
            last_instant = std::time::Instant::now();
            last_len = file_size;
        }
    }
}
