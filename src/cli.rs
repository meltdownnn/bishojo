use structopt::StructOpt;
#[derive(StructOpt)]
pub struct ApplicationMainEntry {
    #[structopt(short = "j", long, default_value = "database.json")]
    pub json_database_location: String,
    #[structopt(short = "b", long, default_value = "database.bin")]
    pub binary_database_location: String,
    #[structopt(short, long)]
    pub proxy: Option<String>,
    #[structopt(short, long)]
    pub thread_limit: Option<usize>,
    #[structopt(
        long = "user-agent",
        default_value = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:53.0) Gecko/20100101 Firefox/53.0"
    )]
    pub user_agent: String,
    #[structopt(long, default_value = "20")]
    pub timeout: u64,
    #[structopt(long, short)]
    pub retry: Option<usize>,
    #[structopt(subcommand)]
    pub subcommand: ApplicationSubCommand,
}
#[derive(StructOpt)]
pub enum ApplicationSubCommand {
    FetchMetadata {
        #[structopt(long)]
        site: AvailableWebsite,
        #[structopt(short, long)]
        start_page: u32,
        #[structopt(short, long)]
        end_page: u32,
        #[structopt(short, long)]
        overwrite: bool
    },
    DownloadUserAvatars {
        #[structopt(long)]
        site: Option<AvailableWebsite>,
        #[structopt(short, long)]
        overwrite: bool,
    },
    DownloadImages {
        #[structopt(long)]
        game_id: Vec<u64>,
        #[structopt(short, long)]
        overwrite: bool,
    },
    DownloadGame {
        #[structopt(long)]
        game_id: Vec<u64>,
        #[structopt(short, long)]
        no_overwrite: bool,
        #[structopt(long, default_value = "Downloads/")]
        download_path: String,
        #[structopt(long, short)]
        save_unparsable_games_list: Option<String>,
    },
    Export {
        #[structopt(short, long)]
        markdown_location: Option<String>,
        #[structopt(short, long)]
        html_location: Option<String>,
        #[structopt(short, long)]
        prefer_online: bool,
    },
}
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum AvailableWebsite {
    KKGal,
}
impl AvailableWebsite {
    pub fn to_struct(&self) -> Box<dyn crate::websites::GalgameWebsite> {
        match self {
            Self::KKGal => Box::new(crate::websites::kkgal::KKGal {}),
        }
    }
}
impl std::str::FromStr for AvailableWebsite {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kkgal" => Ok(Self::KKGal),
            _ => Err("Unknown website"),
        }
    }
}
