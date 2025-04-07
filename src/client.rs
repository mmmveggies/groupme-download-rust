use std::{fmt::Display, time::Duration};

use chrono::{DateTime, Utc};
use futures_core::Stream;
use miette::IntoDiagnostic;
use serde::Deserialize;

use crate::{
    cache::Cache,
    config::Config,
    model::{Group, GroupMessagesResponse, GroupsResponse, Message},
};

#[derive(Clone)]
pub struct Client {
    #[expect(dead_code)]
    cache: Cache,
    config: Config,
}

impl Client {
    /// Instantiate a [`Client`].
    pub fn new(cache: Cache, config: Config) -> Client {
        Self { cache, config }
    }

    /// TODO: only gets the first 100.
    pub async fn get_all_groups(&self) -> miette::Result<Vec<Group>> {
        let mut groups = Vec::new();
        let mut page = 1;

        loop {
            let response = self
                .get::<GroupsResponse>(
                    "/groups",
                    vec![("per_page", Some(10)), ("page", Some(page))],
                )
                .await?;
            if response.response.is_empty() {
                break;
            }

            page += 1;
            groups.extend(response.response);
            if groups.len() > 100 {
                break;
            }
        }

        Ok(groups)
    }

    /// Stream all messages
    pub async fn get_messages(
        &self,
        newest: DateTime<Utc>,
        oldest: DateTime<Utc>,
        group_id: String,
    ) -> miette::Result<impl Stream<Item = miette::Result<Message>>> {
        if newest <= oldest {
            miette::bail!(
                "Newest date {} must be later than oldest date {}",
                newest,
                oldest
            );
        }

        let client = self.clone();
        let mut before_id: Option<String> = None;

        Ok(async_stream::try_stream! {
            loop {
                let messages_page = client.get::<GroupMessagesResponse>(
                    format!("/groups/{group_id}/messages"),
                    vec![
                        ("limit", Some("100".to_string())),
                        ("before_id", before_id),
                    ]
                ).await?.response;

                before_id = messages_page.next_page_before_id();
                if before_id.is_none() {
                    return;
                }

                for message in messages_page.messages {
                    if message.created_at < oldest {
                        // we have gone outside of our filter range
                        return;
                    }
                    if message.created_at > newest {
                        // we are paginating backwards
                        continue;
                    }
                    yield message;
                }

                tokio::time::sleep(Duration::from_secs(1)).await
            }
        })
    }

    /// make a GET request
    async fn get<T>(
        &self,
        path: impl Display,
        query: Vec<(impl Display, Option<impl Display>)>,
    ) -> miette::Result<T>
    where
        for<'de> T: Deserialize<'de>,
    {
        let token = &self.config.api_token;

        let query = query
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| format!("{k}={v}")))
            .chain(std::iter::once(format!("token={token}")))
            .collect::<Vec<_>>()
            .join("&");

        let href = format!("https://api.groupme.com/v3{path}?{query}");

        let bytes = reqwest::get(href)
            .await
            .into_diagnostic()?
            .bytes()
            .await
            .into_diagnostic()?;

        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))
            .into_diagnostic()
    }
}
