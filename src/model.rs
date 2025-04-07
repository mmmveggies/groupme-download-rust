use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Timestamp = DateTime<Utc>;

/// An API response
#[derive(Debug, Deserialize, Serialize)]
pub struct GroupsResponse {
    pub meta: ResponseMeta,
    pub response: Vec<Group>,
}

/// An API response's metadata
#[derive(Debug, Deserialize, Serialize)]
pub struct ResponseMeta {
    pub code: i64,
}

/// A group's definition
#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub creator_user_id: String,
    pub image_url: Option<String>,
    pub share_url: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: Timestamp,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: Timestamp,
    pub members: Vec<GroupMember>,
}

/// A member of a [`Group`]
#[derive(Debug, Deserialize, Serialize)]
pub struct GroupMember {
    pub user_id: String,
    pub nickname: String,
    pub muted: bool,
    pub image_url: String,
}

/// An API response
#[derive(Debug, Deserialize, Serialize)]
pub struct GroupMessagesResponse {
    pub meta: ResponseMeta,
    pub response: GroupMessagesPage,
}

/// A page of [`Message`] in a [`Group`]
#[derive(Debug, Deserialize, Serialize)]
pub struct GroupMessagesPage {
    pub count: i64,
    pub messages: Vec<Message>,
}

impl GroupMessagesPage {
    /// Return the `before_id` for the next page during pagination,
    /// if one exists.
    pub fn next_page_before_id(&self) -> Option<String> {
        self.messages.last().map(|last| last.id.clone())
    }
}

/// A message in a [`Group`]
#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub id: String,
    pub source_guid: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: Timestamp,
    pub user_id: String,
    pub group_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub text: Option<String>,
    pub system: bool,
    /// a list of user IDs
    pub favorited_by: Vec<String>,
    pub attachments: Vec<MessageAttachment>,
}

/// An attachment on a [`Message`]
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageAttachment {
    Image {
        url: String,
    },
    LinkedImage {
        url: String,
    },
    Video {
        url: String,
        preview_url: String,
    },
    File {
        url: String,
    },
    Location {
        lat: String,
        lon: String,
        name: String,
    },
    Split {
        token: String,
    },
    Emoji {
        placeholder: String,
        charmap: Vec<Vec<String>>,
    },
    Reply {
        user_id: String,
        reply_id: String,
        base_reply_id: String,
    },
}

impl MessageAttachment {
    pub fn get_download_url_and_ext(&self) -> Option<(&str, &str)> {
        let url = match self {
            Self::Image { url } => url,
            Self::LinkedImage { url } => url,
            Self::Video { url, .. } => url,
            _ => return None,
        }
        .as_str();

        let ext = if url.contains(".jpeg.") {
            "jpeg"
        } else if url.contains(".png.") {
            "png"
        } else if url.ends_with(".mp4") {
            "mp4"
        } else {
            return None;
        };

        Some((url, ext))
    }
}
