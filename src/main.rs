use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Datelike, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use clap::{Parser, Subcommand};
use dialoguer::{Input, Password, Select};
use futures_util::pin_mut;
use futures_util::stream::StreamExt;
use miette::IntoDiagnostic;

pub mod cache;
pub mod client;
pub mod config;
pub mod model;

use cache::Cache;
use client::Client;
use config::Config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Update your user configuration: set your API Token, and choose your preferred download directory.
    SetConfig,

    /// Download images (requires configuration to be set).
    Download {
        // set start date for the download, otherwise user will be prompted
        #[arg(short, long)]
        start: Option<NaiveDate>,

        // set end date for the download, otherwise user will be prompted
        #[arg(short, long)]
        end: Option<NaiveDate>,
    },
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::SetConfig => {
            let api_token = Password::new()
                .with_prompt("Type or paste your API token here")
                .interact()
                .into_diagnostic()?;

            let config = Config::new(api_token)?;
            Cache::new()?.write_config(&config)?;

            println!("Your configuration has been saved, you can now download images.")
        }
        Command::Download { start, end } => {
            let cache = Cache::new()?;
            let Some(config) = cache.read_config()? else {
                miette::bail!(
                    "User configuration not found. Please use the `set-config` command first."
                )
            };

            let client = Client::new(cache, config.clone());

            let groups = client.get_all_groups().await?;
            let groups_with_readable_names = groups
                .into_iter()
                .map(|group| (format!("{} (group id #{})", group.name, group.id), group))
                .collect::<Vec<_>>();
            let groups_readable_names = groups_with_readable_names
                .iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>();

            let group_idx = Select::new()
                .with_prompt("Select a group to download images from")
                .items(&groups_readable_names)
                .default(0)
                .interact()
                .into_diagnostic()?;

            let (_, group) = groups_with_readable_names
                .get(group_idx)
                .expect("access is checked by Select");

            let group_users = group
                .members
                .iter()
                .map(|user| (&user.user_id, user))
                .collect::<HashMap<_, _>>();

            let now = Local::now();
            let start_date = if let Some(start_date) = start {
                start_date
                    .and_time(NaiveTime::default())
                    .and_local_timezone(Local)
                    .earliest()
                    .expect("Unable to select a start date")
            } else {
                prompt_date(
                    "Enter a start date",
                    round_month(now, -1)
                        .ok_or_else(|| miette::miette!("Unable to select a start date"))?,
                )?
            };

            let end_date = if let Some(end_date) = end {
                end_date
                    .and_time(NaiveTime::default())
                    .and_local_timezone(Local)
                    .earliest()
                    .expect("Unable to select an end date")
            } else {
                prompt_date(
                    "Enter an end date",
                    round_month(now, 0)
                        .ok_or_else(|| miette::miette!("Unable to select an end date"))?,
                )?
            };

            let messages = client
                .get_messages(end_date.to_utc(), start_date.to_utc(), group.id.to_string())
                .await?;

            pin_mut!(messages);
            while let Some(message) = messages.next().await {
                let message = message?;
                let user_name = group_users
                    .get(&message.user_id)
                    .map(|user| user.nickname.as_ref())
                    .unwrap_or_else(|| "unknown");

                let date = message.created_at.with_timezone(&Local);

                for (index, attachment) in message.attachments.iter().enumerate() {
                    let Some((url, ext)) = attachment.get_download_url_and_ext() else {
                        continue;
                    };

                    let filename = format!(
                        "{year}-{month:0>2}-{day:0>2}T{hour:0>2}_{min:0>2}_{sec:0>2}.{index}.{user_name}.{ext}",
                        year = date.year(),
                        month = date.month(),
                        day = date.day(),
                        hour = date.hour(),
                        min = date.minute(),
                        sec = date.second()
                    );
                    let filepath = config.image_dir.join(filename);

                    if fs::exists(&filepath).into_diagnostic()? {
                        println!("file already exists: {filepath:?}");
                        continue;
                    }
                    println!("downloading file: {filepath:?}");

                    let bytes = reqwest::get(url)
                        .await
                        .into_diagnostic()?
                        .bytes()
                        .await
                        .into_diagnostic()?;

                    let mut file = File::options()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(filepath)
                        .into_diagnostic()?;

                    file.write_all(&bytes).into_diagnostic()?;
                }
            }
        }
    }

    Ok(())
}

/// Prompt the user for a YYYY-MM-DD date.
fn prompt_date(prompt: impl Display, default: DateTime<Local>) -> miette::Result<DateTime<Local>> {
    let yyyy_mm_dd: String = Input::new()
        .with_prompt(format!("{prompt} (format YYYY-MM-DD)"))
        .validate_with(|input: &String| {
            if NaiveDate::from_str(input).is_ok() {
                Ok(())
            } else {
                Err("Invalid date format.")
            }
        })
        .default(default.date_naive().to_string())
        .interact()
        .into_diagnostic()?;

    let naive_date = NaiveDate::from_str(&yyyy_mm_dd).into_diagnostic()?;
    let naive_time = NaiveTime::default();
    let naive_datetime = NaiveDateTime::new(naive_date, naive_time);
    naive_datetime
        .and_local_timezone(Local)
        .earliest()
        .ok_or_else(|| miette::miette!("Invalid date."))
}

/// Given a date, round to the beginning of the month, offset by `months` amount
/// of months into the future.
fn round_month(time: DateTime<Local>, months: i8) -> Option<DateTime<Local>> {
    let time = if months < 0 {
        time.checked_sub_months(Months::new(months.unsigned_abs() as u32))
    } else {
        time.checked_add_months(Months::new(months as u32))
    };

    time?
        .with_day(1)?
        .with_time(NaiveTime::default())
        .earliest()
}
