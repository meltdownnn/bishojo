// TODO: add file size support
use structopt::StructOpt;
mod cli;
mod log;
mod saved;
mod websites;
use futures::stream::StreamExt;
use isahc::config::Configurable;
use std::str::FromStr;
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // Using another main in order to make error message better
    _main().await
}
async fn _main() {
    let arguments: cli::ApplicationMainEntry = cli::ApplicationMainEntry::from_args();
    let logging_client = log::LoggingClient::new();
    logging_client.log(
        log::LoggingLevel::Warning,
        &format!(
            "{} ver {} started.",
            std::env!("CARGO_PKG_NAME"),
            std::env!("CARGO_PKG_VERSION")
        ),
    );
    let http_client = isahc::HttpClientBuilder::new()
        .timeout(std::time::Duration::from_secs(arguments.timeout))
        .redirect_policy(isahc::config::RedirectPolicy::Limit(10))
        .auto_referer()
        .tcp_nodelay()
        .proxy(arguments.proxy.map(|x| x.parse().unwrap()))
        .max_connections(arguments.thread_limit.unwrap_or(0))
        .default_header(
            isahc::http::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:53.0) Gecko/20100101 Firefox/53.0",
        )
        .build()
        .unwrap();
    let database = saved::load(
        &arguments.json_database_location,
        &arguments.binary_database_location,
    );
    let mut database = (
        match database.0 {
            Ok(i) => i,
            Err(i) => {
                logging_client.log(
                    log::LoggingLevel::Warning,
                    &format!("Unable to load text database: {}. Using new database.", i),
                );
                Vec::new()
            }
        },
        match database.1 {
            Ok(i) => i,
            Err(i) => {
                logging_client.log(
                    log::LoggingLevel::Warning,
                    &format!("Unable to load binary database: {}. Using new database.", i),
                );
                saved::GameBinaryDatabase(std::collections::HashMap::new())
            }
        },
    );
    match arguments.subcommand {
        cli::ApplicationSubCommand::FetchMetadata {
            site: i,
            start_page: j,
            end_page: k,
            overwrite,
        } => {
            let structure = i.to_struct();
            let immutable_database = database.0.clone();
            let mut job_queue: futures::stream::FuturesUnordered<_> = (j..=k)
                .into_iter()
                .map(|x| {
                    exec_future_and_return_vars(
                        x,
                        structure.fetch_metadata(x, overwrite, &immutable_database, &http_client, &logging_client),
                    )
                })
                .collect();
            while let Some(i) = job_queue.next().await {
                match i.1 {
                    Ok(mut j) => {
                        database.0.append(&mut j);
                        logging_client.log(
                            log::LoggingLevel::StatusReport,
                            &format!("Downloaded metadata from page {}", i.0),
                        );
                    }
                    Err(j) => logging_client.log(
                        log::LoggingLevel::Warning,
                        &format!("Error while downloading metadata in page {}: {}", i.0, j),
                    ),
                }
            }
        }
        cli::ApplicationSubCommand::DownloadUserAvatars { site, overwrite } => {
            let avatars: Vec<(String, cli::AvailableWebsite)> = database
                .0
                .iter()
                .filter(|x| {
                    if let Some(i) = &site {
                        if let Ok(j) = cli::AvailableWebsite::from_str(&x.website) {
                            i == &j
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                })
                .map(|x| {
                    x.comments
                        .iter()
                        .map(|x| x.get_avatars())
                        .flatten()
                        .map(|y| (y, cli::AvailableWebsite::from_str(&x.website).unwrap()))
                        .collect::<Vec<(String, cli::AvailableWebsite)>>()
                })
                .flatten()
                .filter(|x| !(!overwrite && database.1 .0.get(&x.0).is_some()))
                .collect();
            logging_client.log(
                log::LoggingLevel::StatusReport,
                &format!("Will download {} avatars", avatars.len()),
            );
            let avatar_len = avatars.len();
            let mut clients = std::collections::HashMap::new();
            avatars.iter().for_each(|x| {
                clients.insert(x.1, x.1.to_struct());
            });
            let mut job_queue: futures::stream::FuturesUnordered<_> = avatars
                .into_iter()
                .map(|x| {
                    exec_future_and_return_vars(
                        x.0.clone(),
                        clients.get(&x.1).unwrap().download_user_avatars(
                            x.0,
                            &http_client,
                            &logging_client,
                        ),
                    )
                })
                .collect();
            let mut finished_count: usize = 0;
            while let Some(i) = job_queue.next().await {
                finished_count += 1;
                match i.1 {
                    Ok(j) => {
                        database
                            .1
                             .0
                            .insert(i.0.clone(), serde_bytes::ByteBuf::from(j));
                        logging_client.log(
                            log::LoggingLevel::StatusReport,
                            &format!(
                                "Downloaded avatar {} out of {} - {}",
                                finished_count, avatar_len, i.0
                            ),
                        );
                    }
                    Err(j) => {
                        logging_client.log(
                            log::LoggingLevel::Warning,
                            &format!(
                                "Error while downloading {}th avatar ({}): {}",
                                finished_count, i.0, j
                            ),
                        );
                    }
                }
            }
        }
        cli::ApplicationSubCommand::DownloadScreenshots {
            game_id,
            overwrite,
        } => {
            let id_hashmap = match game_id.len() {
                0 => None,
                _ => {
                    let mut hashmap = std::collections::HashMap::new();
                    game_id.into_iter().for_each(|x| {
                        hashmap.insert(x, ());
                    });
                    Some(hashmap)
                }
            };
            let screenshots: Vec<(String, cli::AvailableWebsite, &saved::GameTextInformation)> =
                database
                    .0
                    .iter()
                    .filter(|x| {
                        if let Some(i) = &id_hashmap {
                            i.get(&x.id).is_some()
                        } else {
                            true
                        }
                    })
                    .map(|x| {
                        x.paragraphs
                            .iter()
                            .map(|j| {
                                j.1.iter()
                                    .map(|a| {
                                        (
                                            a,
                                            cli::AvailableWebsite::from_str(&x.website).unwrap(),
                                            x,
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .flatten()
                            .collect::<Vec<_>>()
                    })
                    .flatten()
                    .filter_map(|(x, a, b)| {
                        if let saved::ParagraphContent::Image(i) = x {
                            Some((i.clone(), a, b))
                        } else {
                            None
                        }
                    })
                    .filter(|x| !(database.1 .0.get(&x.0).is_some() && !overwrite))
                    .collect();
            logging_client.log(
                log::LoggingLevel::StatusReport,
                &format!("Will download {} screenshots", screenshots.len()),
            );
            let screenshot_len = screenshots.len();
            let mut clients = std::collections::HashMap::new();
            screenshots.iter().for_each(|x| {
                clients.insert(x.1, x.1.to_struct());
            });
            let mut job_queue: futures::stream::FuturesUnordered<_> = screenshots
                .into_iter()
                .map(|x| {
                    exec_future_and_return_vars(
                        x.0.clone(),
                        clients.get(&x.1).unwrap().download_screenshot(
                            x.0,
                            x.2,
                            &http_client,
                            &logging_client,
                        ),
                    )
                })
                .collect();
            let mut finished_count = 0;
            while let Some(i) = job_queue.next().await {
                finished_count += 1;
                match i.1 {
                    Ok(j) => {
                        database
                            .1
                             .0
                            .insert(i.0.clone(), serde_bytes::ByteBuf::from(j));
                        logging_client.log(
                            log::LoggingLevel::StatusReport,
                            &format!(
                                "Downloaded screenshot {} out of {} - {}",
                                finished_count, screenshot_len, i.0
                            ),
                        );
                    }
                    Err(j) => {
                        logging_client.log(
                            log::LoggingLevel::Warning,
                            &format!(
                                "Error while downlaoding {}th avatar ({}): {}",
                                finished_count, i.0, j
                            ),
                        );
                    }
                }
            }
        }
        cli::ApplicationSubCommand::DownloadGame {
            game_id,
            no_overwrite,
            download_path,
            cache_size,
        } => {
            let mut id_hashmap = std::collections::HashMap::new();
            game_id.into_iter().for_each(|x| {
                id_hashmap.insert(x, ());
            });
            let illegal_character_check =
                |x: char| !(x.is_numeric() | x.is_alphabetic() | x.is_whitespace());
            if !std::path::Path::new(&download_path).is_dir() {
                std::fs::create_dir(&download_path).unwrap();
            }
            let games: Vec<((String, (String, Option<u128>)), String, &saved::GameTextInformation)> = database
                .0
                .iter()
                .filter(|x| id_hashmap.get(&x.id).is_some())
                .map(|x| {
                    x.files
                        .iter()
                        .map(|y| (y, x.name.clone(), x))
                        .collect::<Vec<_>>()
                })
                .flatten()
                .map(|x| {
                    (
                        (
                            x.0 .0.replace(illegal_character_check, "_"),
                            x.0 .1.to_owned(),
                        ),
                        x.1.replace(illegal_character_check, "_"),
                        x.2,
                    )
                })
                .filter(|x| {
                    no_overwrite
                        && std::path::Path::new(&format!("{}{}/{}", download_path, x.1, x.0 .0))
                            .is_file()
                })
                .collect();
            games.iter().for_each(|x| {
                if !std::path::Path::new(&format!("{}{}", download_path, x.1)).is_dir() {
                    std::fs::create_dir(&format!("{}{}", download_path, x.1)).unwrap()
                }
            });
            let mut clients = std::collections::HashMap::new();
            games.iter().for_each(|x| {
                let website = cli::AvailableWebsite::from_str(&x.2.website).unwrap();
                clients.insert(website.to_owned(), website.to_struct());
            });
            let mut job_queue: futures::stream::FuturesUnordered<_> = games
                .into_iter()
                .map(|x| {
                    exec_future_and_return_vars(
                        (x.0 .0.clone(), x.1.clone()),
                        clients
                            .get(&cli::AvailableWebsite::from_str(&x.2.website).unwrap())
                            .unwrap()
                            .download_game(
                                x.0 .1.0,
                                x.2,
                                format!("{}{}/{}", download_path, x.1, x.0 .0),
                                x.0.1.1,
                                &http_client,
                                &logging_client,
                                cache_size.get_bytes() as usize,
                            ),
                    )
                })
                .collect();
            while let Some(i) = job_queue.next().await {
                match i.1 {
                    Ok(()) => logging_client.log(
                        log::LoggingLevel::StatusReport,
                        &format!("Downloaded game {}/{}", i.0 .0, i.0 .1),
                    ),
                    Err(j) => logging_client.log(
                        log::LoggingLevel::Warning,
                        &format!("Error while downloading game {}/{}: {}", i.0 .0, i.0 .1, j),
                    ),
                }
            }
        }
        cli::ApplicationSubCommand::Export {
            markdown_location,
            html_location,
            prefer_online,
        } => {
            if let Some(_) = html_location {
                unimplemented!()
            }
            if let Some(i) = markdown_location {
                if !std::path::Path::new(&i).is_dir() {
                    std::fs::create_dir_all(format!("{}/imgs", i)).unwrap();
                }
                let generated_pages = markdown_generator(
                    &database.0,
                    if prefer_online {
                        None
                    } else {
                        Some(&database.1)
                    },
                );
                logging_client.log(log::LoggingLevel::Message, "Markdown pages exported.");
                let mut job_vec = Vec::new();
                for page in generated_pages {
                    job_vec.push(exec_future_and_return_vars(
                        (true, page.0.clone()),
                        tokio::fs::write(format!("{}/{}.md", i, page.0), page.1.into_bytes()),
                    ));
                }
                for data in &database.1 .0 {
                    let data: (&String, &serde_bytes::ByteBuf) = data;
                    let suffix_position = data.0.rfind('.').unwrap_or(data.0.len() - 1);
                    job_vec.push(exec_future_and_return_vars(
                        (false, data.0.clone()),
                        tokio::fs::write(
                            format!(
                                "{}/imgs/{}.{}",
                                i,
                                &data.0[..suffix_position],
                                &data.0[suffix_position + 1..]
                            ),
                            data.1.to_vec(),
                        ),
                    ));
                }
                let mut job_queue: futures::stream::FuturesUnordered<_> =
                    job_vec.into_iter().collect();
                while let Some(i) = job_queue.next().await {
                    if let Err(j) = i.1 {
                        logging_client.log(
                            log::LoggingLevel::Warning,
                            &format!("Error while writing {} to disk: {}", i.0 .1, j.to_string()),
                        );
                    } else {
                        logging_client.log(
                            log::LoggingLevel::StatusReport,
                            &format!(
                                "{} {} written.",
                                i.0 .1,
                                if i.0 .0 { "markdown" } else { "image" }
                            ),
                        );
                    }
                }
            }
        }
    }
    saved::save(
        (&database.0, &arguments.json_database_location),
        (&database.1, &arguments.binary_database_location),
    )
    .unwrap();
    logging_client.log(log::LoggingLevel::Warning, "Database synced.")
}
async fn exec_future_and_return_vars<T, U: std::future::Future>(
    vars: T,
    function: U,
) -> (T, U::Output) {
    (vars, function.await)
}
fn markdown_generator(
    database: &saved::GameTextDatabase,
    offline_data: Option<&saved::GameBinaryDatabase>,
) -> Vec<(String, String)> {
    let mut constructed = Vec::new();
    fn find_offline_data_or_use_remove(
        x: String,
        offline_data: Option<&saved::GameBinaryDatabase>,
    ) -> String {
        if let Some(data) = offline_data {
            if data.0.get(&x).is_some() {
                let suffix_position = x.rfind('.').unwrap_or(x.len() - 1);
                format!(
                    "imgs/{}.{}",
                    hash_for_filename(&x[..suffix_position]),
                    &x[suffix_position + 1..]
                )
            } else {
                x
            }
        } else {
            x
        }
    };
    for game in database {
        let mut game_detail = format!("# {}\n\n", game.name);
        let game_name = format!(
            "{}_{}",
            game.name.replace(
                |x: char| !(x.is_whitespace() | x.is_alphabetic() | x.is_numeric()),
                "_"
            ),
            game.id.to_string()
        );
        game_detail.push_str(&format!(
            "> id: {}@{} | published at {} | {} viewed\n\n",
            game.id.to_string(),
            game.website,
            game.published.to_string(),
            game.viewed
        ));
        game_detail.push_str("Tags: ");
        game_detail.push_str(&game.tags.join(", "));
        game_detail.push_str("\n\n");
        for paragraph in &game.paragraphs {
            let title = if let Some(i) = &paragraph.0 {
                format!("## {}", i)
            } else {
                String::from("---")
            };
            let mut content_rendered = String::new();
            for content in &paragraph.1 {
                match content {
                    saved::ParagraphContent::Text(i) => content_rendered.push_str(i),
                    saved::ParagraphContent::Image(i) => content_rendered.push_str(&format!(
                        "![image]({})",
                        find_offline_data_or_use_remove(i.to_string(), offline_data)
                    )),
                }
                content_rendered.push('\n')
            }
            game_detail.push_str(&format!("{}\n{}\n\n", title, content_rendered));
        }
        game_detail.push_str(&format!(
            "## Downloads\n{}\n\n",
            game.files
                .iter()
                .map(|x| format!("- {}\n  {} {}", x.0, x.1.0, if let Some(i) = x.1.1 {byte_unit::Byte::from_bytes(i).to_string()} else {String::new()}))
                .collect::<Vec<_>>()
                .join("\n")
        ));
        fn comments_constructor(
            comment: &saved::Comment,
            offline_data: Option<&saved::GameBinaryDatabase>,
        ) -> String {
            let avatar_link = if let Some(data) = offline_data {
                if data.0.get(&comment.user_avatar).is_some() {
                    let avatar_suffix_location = comment
                        .user_avatar
                        .rfind('.')
                        .unwrap_or(comment.user_avatar.len() - 1);
                    format!(
                        "imgs/{}.{}",
                        hash_for_filename(&comment.user_avatar[..avatar_suffix_location]),
                        &comment.user_avatar[avatar_suffix_location + 1..]
                    )
                } else {
                    comment.user_avatar.clone()
                }
            } else {
                comment.user_avatar.clone()
            };
            let mut content = String::new();
            for i in &comment.content {
                match i {
                    saved::ParagraphContent::Text(i) => content.push_str(&i),
                    saved::ParagraphContent::Image(i) => content.push_str(&format!(
                        "![image]({})",
                        find_offline_data_or_use_remove(i.to_string(), offline_data)
                    )),
                }
                content.push('\n');
            }
            let formatted = format!(
                "\n![avatar]({}) {} said on {}:\n{}\n{}",
                avatar_link,
                comment.author,
                comment.date.to_string(),
                content,
                comment
                    .replies
                    .iter()
                    .map(|x| comments_constructor(x, offline_data))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            formatted.replace("\n", "\n> ")
        };
        game_detail.push_str(&format!(
            "## Comments\n{}\n\n",
            game.comments
                .iter()
                .map(|x| comments_constructor(x, offline_data))
                .collect::<Vec<_>>()
                .join("\n")
        ));
        constructed.push((game_name, game_detail))
    }
    constructed
}
fn hash_for_filename(text: &str) -> String {
    let text = if text.len() > 64 {
        &text[text.len() - 65..text.len()]
    } else {
        text
    };
    base64::encode_config(text, base64::URL_SAFE_NO_PAD)
}
