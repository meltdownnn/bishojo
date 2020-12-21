use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::HashMap;
pub type GameTextDatabase = Vec<GameTextInformation>;
#[derive(Serialize, Deserialize)]
pub struct GameTextInformation {
    pub id: u64,
    pub website: String,
    pub name: String,
    pub tags: Vec<String>,
    pub published: time::PrimitiveDateTime,
    pub viewed: u128,
    pub paragraphs: Vec<(Option<String>, Vec<ParagraphContent>)>,
    pub miscellaneous: HashMap<String, String>,
    pub files: Vec<(String, (String, Option<u128>))>,
    pub comments: Vec<Comment>,
}
impl GameTextInformation {
    pub fn default(id: u64, website: String) -> Self {
        Self {
            id,
            website,
            name: String::default(),
            tags: Vec::default(),
            published: time::PrimitiveDateTime::new(time::date!(2019 - 01 - 01), time::time!(0:00)),
            viewed: 0,
            paragraphs: Vec::default(),
            miscellaneous: std::collections::HashMap::new(),
            files: Vec::default(),
            comments: Vec::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum ParagraphContent {
    Text(String),
    Image(String),
}
#[derive(Serialize, Deserialize)]
pub struct Comment {
    pub user_avatar: String,
    pub author: String,
    pub date: time::PrimitiveDateTime,
    pub content: Vec<ParagraphContent>,
    pub replies: Vec<Comment>,
}
impl Comment {
    pub fn get_avatars(&self) -> Vec<String> {
        let mut avatars = vec![self.user_avatar.to_owned()];
        for i in &self.replies {
            avatars.append(&mut i.get_avatars());
        }
        avatars
    }
    // This should not be used since we handle it directly in export code now
    pub fn _replace_avatars(&mut self, hashmap: &std::collections::HashMap<String, String>) {
        if let Some(i) = hashmap.get(&self.user_avatar) {
            self.user_avatar = i.to_string();
        }
        for i in &mut self.replies {
            i._replace_avatars(hashmap)
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct GameBinaryDatabase(pub HashMap<String, ByteBuf>);
pub fn load(
    text_db: &str,
    binary_db: &str,
) -> (
    Result<GameTextDatabase, String>,
    Result<GameBinaryDatabase, String>,
) {
    let text_db = std::fs::read(text_db)
        .map_err(|x| x.to_string())
        .and_then(|x| serde_json::from_slice(&x).map_err(|x| x.to_string()));
    let binary_db = std::fs::read(binary_db)
        .map_err(|x| x.to_string())
        .and_then(|x| rmp_serde::from_slice(&x).map_err(|x| x.to_string()));
    (text_db, binary_db)
}
pub fn save(
    text_db: (&GameTextDatabase, &str),
    binary_db: (&GameBinaryDatabase, &str),
) -> Result<(), String> {
    std::fs::write(
        text_db.1,
        serde_json::to_vec_pretty(text_db.0).map_err(|x| x.to_string())?,
    )
    .map_err(|x| x.to_string())?;
    std::fs::write(
        binary_db.1,
        rmp_serde::to_vec(binary_db.0).map_err(|x| x.to_string())?,
    )
    .map_err(|x| x.to_string())
}
