use async_trait::async_trait;
use futures::io::AsyncReadExt;
use futures::stream::StreamExt;
use isahc::prelude::*;
pub const WEBSITE_LINK: &'static str = "https://www.kkgal.com";
pub struct KKGal {}
impl KKGal {
    pub async fn download_file_information(
        url: &str,
        client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Vec<(String, (String, Option<u128>))>, String> {
        #[derive(serde::Deserialize)]
        struct DownloadedFileStructure {
            pub name: String,
            pub size: String,
            pub date: String,
        }
        let mut response_content;
        let parsed_html = loop {
            let mut response_body = client
                .send_async(
                    Request::get(url)
                        .header(isahc::http::header::REFERER, WEBSITE_LINK)
                        .body(())
                        .map_err(|x| x.to_string())?,
                )
                .await
                .map_err(|x| x.to_string())?
                .into_body();
            let mut response_vec = Vec::new();
            response_body
                .read_to_end(&mut response_vec)
                .await
                .map_err(|x| x.to_string())?;
            response_content = String::from_utf8_lossy(&response_vec).to_string();
            match Self::pass_groot_verify(&response_content, client, log_client).await {
                Ok(u) => {
                    if let Some(i) = u {
                        break i;
                    }
                }
                Err(i) => return Err(i),
            }
        };
        let script = parsed_html
            .get(
                parsed_html
                    .iter()
                    .enumerate()
                    .find(|x| x.1 == &rusthtml::HtmlTag::OpeningTag("script", vec![]))
                    .ok_or(String::from("Error while finding script"))?
                    .0
                    + 1,
            )
            .ok_or(String::from("Error while reading script"))?;
        let script = if let rusthtml::HtmlTag::Unparsable(i) = script {
            i
        } else {
            return Err(String::from("Assertion error: script should be a text"));
        };
        let text = {
            let text_start = script
                .find("rawData")
                .ok_or(String::from("Error while finding rawData of the script"))?;
            let text = &script[text_start..];
            let text_start = text.find("\"").ok_or(String::from(
                "Error while finding start position of the script",
            ))?;
            let text = &text[text_start + 1..];
            let text_end = text.find("\"").ok_or(String::from(
                "Error while finding end position of the script",
            ))?;
            text[..text_end].to_string()
        };
        let text = unescape::unescape(&text).ok_or(String::from("Error while unescaping quote"))?;
        let text =
            base64::decode(&text).map_err(|x| format!("Error while parsing script: {}", x))?;
        let parsed_detail: Vec<DownloadedFileStructure> = serde_json::from_slice(&text)
            .map_err(|x| format!("Error while deserializing script: {}", x))?;
        let mut constructed = Vec::new();
        for detail in parsed_detail {
            let mut site_link = format!("{}/{}", url, &detail.name);
            let mut date_and_time: std::str::SplitWhitespace = detail.date.split_whitespace();
            let mut date = date_and_time
                .next()
                .ok_or(String::from("Error while parsing date from file detail"))?
                .split('/');
            let mut time = date_and_time
                .next()
                .ok_or(String::from("Error while parsing time from file detail"))?
                .split(':');
            let _date = time::Date::try_from_ymd(
                date.next()
                    .ok_or("Bad date".to_string())?
                    .parse()
                    .map_err(|_| String::from("Error while parsing year"))?,
                date.next()
                    .ok_or("Bad date".to_string())?
                    .parse()
                    .map_err(|_| String::from("Error while parsing month"))?,
                date.next()
                    .ok_or("Dad date".to_string())?
                    .parse()
                    .map_err(|_| String::from("Error while parsing day"))?,
            )
            .map_err(|x| {
                format!(
                    "Error while parsing date from file detail: {}",
                    x.to_string()
                )
            })?
            .try_with_hms(
                time.next()
                    .ok_or("Bad time".to_string())?
                    .parse()
                    .map_err(|_| String::from("Error while parsing hour"))?,
                time.next()
                    .ok_or("Bad time".to_string())?
                    .parse()
                    .map_err(|_| String::from("Error while parsing minute"))?,
                time.next()
                    .ok_or("Bad time".to_string())?
                    .parse()
                    .map_err(|_| String::from("Error while parsing second"))?,
            )
            .map_err(|x| x.to_string())?;
            //log_client.log(
            //    crate::log::LoggingLevel::StatusReport,
            //    &format!(
            //        "Found downloadable link {} ({}, {})",
            //        site_link,
            //        detail.size,
            //        date.to_string()
            //    ),
            //);
            if std::env::var_os("USE_DIRECT").is_some() {
                if let Ok(i) = client.send_async(Request::get(&site_link).redirect_policy(isahc::config::RedirectPolicy::None).header(isahc::http::header::REFERER, WEBSITE_LINK).body(()).map_err(|x| x.to_string())?).await {
                    println!("{:?}", i.headers());
                    site_link = i.headers().get("location").map(|x| x.to_str().map(|x| x.to_string()).unwrap_or(site_link.to_string())).unwrap_or(site_link)
                }
            }
            let size = byte_unit::Byte::from_str(&detail.size)
                .ok()
                .map(|x| x.get_bytes());
            constructed.push((percent_encoding::percent_decode_str(&detail.name).decode_utf8_lossy().to_string(), (site_link, size)));
        }
        Ok(constructed)
    }
    async fn pass_groot_verify<'a>(
        response: &'a str,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Option<Vec<rusthtml::HtmlTag<'a>>>, String> {
        let response_parsed = rusthtml::HtmlTag::parse(response);
        if response_parsed
            .iter()
            .find(|x| &&rusthtml::HtmlTag::Unparsable("I'm Groot") == x)
            .is_none()
        {
            return Ok(Some(response_parsed));
        }
        for i in response_parsed {
            if let rusthtml::HtmlTag::OpeningTag("a", j) = i {
                let link = j
                    .iter()
                    .find(|x| x.0 == "href")
                    .ok_or(String::from("Unknown link format when passing groot"))?
                    .1
                    .ok_or(String::from("Link without argument when passing groot"))?;
                http_client
                    .send_async(
                        Request::get(&format!("{}{}", WEBSITE_LINK, link))
                            .header(isahc::http::header::CONNECTION, "keep-alive")
                            .body(())
                            .map_err(|x| x.to_string())?,
                    )
                    .await
                    .map_err(|x| x.to_string())?;
                log_client.log(crate::log::LoggingLevel::StatusReport, "Groot Bypassed.");
                return Ok(None);
            }
        }
        Err(String::from("Groot detected, but cannot find link"))
    }
    pub async fn download_index(
        page: u32,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Vec<String>, String> {
        let mut content_text;
        let mut parsed_html = loop {
            let mut content = http_client
                .send_async(
                    Request::get(&format!("{}/page/{}/", WEBSITE_LINK, page.to_string()))
                        .header(isahc::http::header::CONNECTION, "keep-alive")
                        .body(())
                        .map_err(|x| x.to_string())?,
                )
                .await
                .map_err(|x| x.to_string())?
                .into_body();
            let mut content_text_vec = Vec::new();
            content
                .read_to_end(&mut content_text_vec)
                .await
                .map_err(|x| x.to_string())?;
            content_text = String::from_utf8_lossy(&content_text_vec).to_string();
            match Self::pass_groot_verify(&content_text, http_client, log_client).await {
                Ok(i) => {
                    if let Some(i) = i {
                        break i;
                    }
                }
                Err(i) => {
                    return Err(i);
                }
            }
        };
        let mut found_links = Vec::new();
        while parsed_html.len() != 0 {
            match parsed_html.remove(0) {
                rusthtml::HtmlTag::OpeningTag("div", attributes) => {
                    if attributes == vec![("class", Some("title-article"))] {
                        if let rusthtml::HtmlTag::OpeningTag("h1", _) = parsed_html.remove(0) {
                            if let rusthtml::HtmlTag::OpeningTag("a", attributes) =
                                parsed_html.remove(0)
                            {
                                found_links.push(
                                    attributes
                                        .into_iter()
                                        .filter(|x| x.0 == "href")
                                        .map(|x| x.1.unwrap().to_string())
                                        .next()
                                        .ok_or(String::from("Bad link"))?,
                                );
                            }
                        }
                    }
                }
                _ => continue,
            }
        }
        log_client.log(
            crate::log::LoggingLevel::StatusReport,
            &format!("Found {} links on page {}", found_links.len(), page),
        );
        Ok(found_links)
    }
    pub async fn download_information(
        url_owned: String,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<crate::saved::GameTextInformation, String> {
        let url: &str = &url_owned;
        let mut response_text;
        let mut parsed_html = loop {
            let mut response = http_client
                .send_async(
                    Request::get(url)
                        .header(isahc::http::header::CONNECTION, "keep-alive")
                        .body(())
                        .map_err(|x| x.to_string())?,
                )
                .await
                .map_err(|x| x.to_string())?
                .into_body();
            let mut response_vec = Vec::new();
            response
                .read_to_end(&mut response_vec)
                .await
                .map_err(|x| x.to_string())?;
            response_text = String::from_utf8_lossy(&response_vec).to_string();
            match Self::pass_groot_verify(&response_text, http_client, log_client).await {
                Ok(i) => {
                    if let Some(i) = i {
                        break i;
                    }
                }
                Err(i) => {
                    return Err(i);
                }
            }
        };
        let mut constructed = crate::saved::GameTextInformation::default(
            seahash::hash(url.as_bytes()),
            String::from("kkgal"),
        );
        let mut screenshots = Vec::new();
        while parsed_html.len() != 0 {
            let current_element = parsed_html.remove(0);
            match current_element {
                rusthtml::HtmlTag::OpeningTag("meta", mut attributes) => {
                    if attributes.get(1) == Some(&("name", Some("keywords"))) {
                        constructed.tags = attributes
                            .remove(0)
                            .1
                            .ok_or(String::from("Unable to parse keywords"))?
                            .split(", ")
                            .map(|x| x.to_string())
                            .collect::<Vec<String>>();
                        // The last tag is empty
                        constructed.tags.remove(constructed.tags.len() - 1);
                    }
                }
                rusthtml::HtmlTag::OpeningTag("a", attributes) => {
                    if attributes == vec![("href", Some(url))] {
                        loop {
                            if let rusthtml::HtmlTag::Unparsable(i) = parsed_html.remove(0) {
                                constructed.name = i.to_string();
                                break;
                            }
                        }
                    }
                }
                rusthtml::HtmlTag::OpeningTag("i", attributes) => {
                    if attributes == vec![("class", Some("fa fa-calendar"))] {
                        loop {
                            if let rusthtml::HtmlTag::Unparsable(i) = parsed_html.remove(0) {
                                let mut time_split = i.trim().split('-');
                                constructed.published = time::Date::try_from_ymd(
                                    time_split
                                        .next()
                                        .ok_or(String::from("Unable to parse year"))?
                                        .parse()
                                        .map_err(|_| String::from("Error while parsing year"))?,
                                    time_split
                                        .next()
                                        .ok_or(String::from("Unable to parse month"))?
                                        .parse()
                                        .map_err(|_| String::from("Error while parsing month"))?,
                                    time_split
                                        .next()
                                        .ok_or(String::from("Unable to parse day"))?
                                        .parse()
                                        .map_err(|_| String::from("Error while parsing day"))?,
                                )
                                .map_err(|x| {
                                    format!("Error while parsing date: {}", x.to_string())
                                })?
                                .with_time(time::time!(0:00));
                                break;
                            }
                        }
                    } else if attributes == vec![("class", Some("fa fa-eye"))] {
                        loop {
                            if let rusthtml::HtmlTag::Unparsable(i) = parsed_html.remove(0) {
                                constructed.viewed = i
                                    .trim()
                                    .trim_end_matches("℃")
                                    .trim()
                                    .replace(',', "")
                                    .parse()
                                    .map_err(|_| String::from("Error while parsing viewed"))?;
                                break;
                            }
                        }
                    }
                }
                rusthtml::HtmlTag::OpeningTag("img", mut attributes) => {
                    if attributes.get(0) == Some(&("title", Some("点击放大"))) {
                        screenshots.push(crate::saved::ParagraphContent::Image(
                            attributes
                                .remove(1)
                                .1
                                .ok_or(String::from("Unable to parse image"))?
                                .to_string(),
                        ))
                    }
                }
                rusthtml::HtmlTag::OpeningTag("span", attributes) => {
                    if attributes == vec![("style", Some("font-size: 12pt;COLOR:#3399CC"))] {
                        loop {
                            if let rusthtml::HtmlTag::Unparsable(i) = parsed_html.remove(0) {
                                let span_title = i.to_string();
                                let mut span_content = String::new();
                                loop {
                                    match parsed_html.remove(0) {
                                        rusthtml::HtmlTag::Unparsable(i) => {
                                            span_content.push_str(i)
                                        }
                                        rusthtml::HtmlTag::OpeningTag("br", _) => {
                                            span_content.push('\n')
                                        }
                                        rusthtml::HtmlTag::ClosingTag("div") => break,
                                        _ => continue,
                                    }
                                }
                                // TODO: handle image
                                constructed.paragraphs.push((
                                    Some(span_title),
                                    vec![crate::saved::ParagraphContent::Text(span_content)],
                                ));
                                break;
                            }
                        }
                    } else if attributes == vec![("style", Some("font-size: 11pt;"))] {
                        let mut span_content = String::new();
                        loop {
                            match parsed_html.remove(0) {
                                rusthtml::HtmlTag::Unparsable(i) => span_content.push_str(i),
                                rusthtml::HtmlTag::OpeningTag("br", _) => span_content.push('\n'),
                                rusthtml::HtmlTag::ClosingTag("span") => break,
                                _ => continue,
                            }
                        }
                        // TODO: handle image
                        constructed.paragraphs.push((
                            None,
                            vec![crate::saved::ParagraphContent::Text(span_content)],
                        ));
                    }
                }
                rusthtml::HtmlTag::OpeningTag("button", mut attributes) => {
                    if attributes.get(0) == Some(&("type", Some("button")))
                        && attributes.get(1) == Some(&("class", Some("btn btn-danger")))
                    {
                        let link = attributes
                            .remove(2)
                            .1
                            .ok_or(String::from("Unable to parse download link"))?;
                        constructed.miscellaneous.insert(
                            String::from("overall_link"),
                            String::from(
                                &link[link.find('\'').ok_or(String::from(
                                    "Unable to find starting position for download link",
                                ))? + 1
                                    ..link.rfind('\'').ok_or(String::from(
                                        "Unable to find ending position for download link",
                                    ))?],
                            ),
                        );
                    }
                }
                rusthtml::HtmlTag::OpeningTag("ol", attributes) => {
                    if attributes == vec![("class", Some("commentlist"))] {
                        let mut comments: Vec<crate::saved::Comment> = Vec::new();
                        let mut current_depth = 0;
                        loop {
                            match parsed_html.remove(0) {
                                rusthtml::HtmlTag::OpeningTag("li", _) => {
                                    let avatar = loop {
                                        if let rusthtml::HtmlTag::OpeningTag("img", attributes) =
                                            parsed_html.remove(0)
                                        {
                                            break attributes;
                                        }
                                    }
                                    .into_iter()
                                    .filter(|x| x.0 == "src")
                                    .map(|x| {
                                        format!(
                                            "{}{}",
                                            WEBSITE_LINK,
                                            x.1.ok_or(String::from("Unable to parse avatar"))
                                                .unwrap()
                                        )
                                    })
                                    .next()
                                    .ok_or(String::from("Unable to find avatar"))?;
                                    let name = loop {
                                        if let rusthtml::HtmlTag::OpeningTag("b", _) =
                                            parsed_html.remove(0)
                                        {
                                            break loop {
                                                if let rusthtml::HtmlTag::Unparsable(i) =
                                                    parsed_html.remove(0)
                                                {
                                                    break i;
                                                }
                                            };
                                        }
                                    }
                                    .to_string();
                                    let time = time::PrimitiveDateTime::parse(
                                        loop {
                                            if let rusthtml::HtmlTag::OpeningTag("time", i) =
                                                parsed_html.remove(0)
                                            {
                                                break i;
                                            }
                                        }
                                        .into_iter()
                                        .filter(|x| x.0 == "datetime")
                                        .map(|x| x.1.unwrap().to_string())
                                        .next()
                                        .ok_or(String::from("Unable to parse time"))?,
                                        "%FT%T+00:00",
                                    )
                                    .map_err(|x| format!("Error while parsing time: {}", x))?;
                                    loop {
                                        if let rusthtml::HtmlTag::OpeningTag("div", i) =
                                            parsed_html.remove(0)
                                        {
                                            if i == vec![("class", Some("comment-content"))] {
                                                break;
                                            }
                                        }
                                    }
                                    // TODO: handle image
                                    let mut content = String::new();
                                    loop {
                                        match parsed_html.remove(0) {
                                            rusthtml::HtmlTag::OpeningTag("br", _) => {
                                                content.push('\n')
                                            }
                                            rusthtml::HtmlTag::Unparsable(i) => content.push_str(i),
                                            rusthtml::HtmlTag::ClosingTag("div") => break,
                                            _ => continue,
                                        }
                                    }
                                    let mut reference = &mut comments;
                                    for _ in 0..current_depth {
                                        reference = &mut reference.last_mut().unwrap().replies;
                                    }
                                    reference.push(crate::saved::Comment {
                                        user_avatar: avatar,
                                        author: name,
                                        date: time,
                                        content: vec![crate::saved::ParagraphContent::Text(
                                            content,
                                        )],
                                        replies: Vec::new(),
                                    });
                                    current_depth += 1;
                                }
                                rusthtml::HtmlTag::ClosingTag("li") => current_depth -= 1,
                                rusthtml::HtmlTag::ClosingTag("ol") => break,
                                _ => continue,
                            }
                        }
                        constructed.comments = comments;
                    }
                }
                _ => continue,
            }
        }
        if let Some(i) = constructed.miscellaneous.get("overall_link") {
            if i.find("pan.baidu.com").is_none() {
                match Self::download_file_information(i, http_client, log_client).await {
                    Ok(i) => constructed.files = i,
                    Err(j) => {
                        log_client.log(
                            crate::log::LoggingLevel::Warning,
                            &format!(
                                "Error while parsing download url: {}, storing it as pure text.",
                                j
                            ),
                        );
                        constructed.paragraphs.push((
                            Some(String::from("Download link")),
                            vec![crate::saved::ParagraphContent::Text(i.to_string())],
                        ))
                    }
                }
            } else {
                constructed.paragraphs.push((
                    Some(String::from("Download link")),
                    vec![crate::saved::ParagraphContent::Text(i.to_string())],
                ))
            }
        }
        constructed
            .paragraphs
            .push((Some(String::from("Screenshots")), screenshots));
        log_client.log(
            crate::log::LoggingLevel::StatusReport,
            &format!(
                "Parsed information from {}(id: {})",
                constructed.name, constructed.id
            ),
        );
        Ok(constructed)
    }
}

#[async_trait]
impl super::GalgameWebsite for KKGal {
    async fn fetch_metadata(
        &self,
        page: u32,
        overwrite: bool,
        database: &crate::saved::GameTextDatabase,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
    ) -> Result<Vec<crate::saved::GameTextInformation>, String> {
        let metadata = Self::download_index(page, http_client, log_client).await?;
        let mut job_vec = Vec::new();
        for i in metadata {
            if !overwrite
                && database
                    .iter()
                    .find(|x| x.id == seahash::hash(i.as_bytes()))
                    .is_some()
            {
                continue;
            }
            job_vec.push(crate::exec_future_and_return_vars(
                i.clone(),
                Self::download_information(i.clone(), http_client, log_client),
            ));
        }
        let mut job_queue: futures::stream::FuturesUnordered<_> = job_vec.into_iter().collect();
        let mut result_vec = Vec::new();
        while let Some(i) = job_queue.next().await {
            match i.1 {
                Ok(j) => result_vec.push(j),
                Err(j) => log_client.log(
                    crate::log::LoggingLevel::Warning,
                    &format!("Error while parsing information from page {}: {}", i.0, j),
                ),
            }
        }
        Ok(result_vec)
    }
    async fn download_user_avatars(
        &self,
        avatar_url: String,
        http_client: &isahc::HttpClient,
        _: &crate::log::LoggingClient,
    ) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        http_client
            .send_async(
                Request::get(avatar_url)
                    .header(isahc::http::header::CONNECTION, "keep-alive")
                    .body(())
                    .map_err(|x| x.to_string())?,
            )
            .await
            .map_err(|x| x.to_string())?
            .into_body()
            .read_to_end(&mut buffer)
            .await
            .map_err(|x| x.to_string())?;
        Ok(buffer)
    }
    async fn download_screenshot(
        &self,
        screenshot_url: String,
        _: &crate::saved::GameTextInformation,
        http_client: &isahc::HttpClient,
        _: &crate::log::LoggingClient,
    ) -> Result<Vec<u8>, String> {
        let mut result = Vec::new();
        http_client
            .get_async(screenshot_url)
            .await
            .map_err(|x| x.to_string())?
            .into_body()
            .read_to_end(&mut result)
            .await
            .map_err(|x| x.to_string())?;
        Ok(result)
    }
    // TODO: add direct link support
    async fn download_game(
        &self,
        link: String,
        _: &crate::saved::GameTextInformation,
        file: String,
        predicted_size: Option<u128>,
        http_client: &isahc::HttpClient,
        log_client: &crate::log::LoggingClient,
        cache_size: usize,
    ) -> Result<(), String> {
        let response = http_client.get_async(link);
        super::game_download_helper(response, &file, log_client, cache_size, predicted_size).await
    }
}
