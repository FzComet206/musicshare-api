use crate::utils::error::{ Error, Result };
use std::path::Path;
use std::fs::File;
use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task;
use futures::stream::{FuturesUnordered, StreamExt};
use sqlx::PgPool;
use sqlx::Row;
use dotenvy::dotenv;
use std::env;

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{config::Region, Client};

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use std::collections::HashMap;

const COOKIES_PATH: &str = "./libs/cookies.txt";
const YT_DLP_PATH: &str = "./libs/yt-dlp";
const FFMPEG_PATH: &str = "./libs/ffmpeg";

pub struct FMDownloadParams{
    pub url: String,
    pub title: String,
    pub userid: String,
    pub pool: PgPool,
}

#[derive(Clone, Debug)]
pub struct FileManager {
    pub semaphore: Arc<Semaphore>,
    pub max_file_size: u64,
    pub s3_client: Client,
    pub processing_user: Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>,
}

impl FileManager {
    pub async fn new() -> Result<(Self)> {

        dotenv().ok();
        let max_concurrent_downloads = env::var("MAX_CONCURRENT_TASKS")
            .unwrap_or("4".to_string())
            .parse::<usize>()
            .expect("MAX_CONCURRENT_TASKS must be a number");

        let semaphore = Arc::new(Semaphore::new(max_concurrent_downloads));

        let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"));
        let region = region_provider.region().await.unwrap();
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        Ok(Self {
            semaphore,
            max_file_size: env::var("MAX_FILE_SIZE")
                .unwrap_or("10000000".to_string())
                .parse::<u64>()
                .expect("MAX_FILE_SIZE must be a number"),
            s3_client: client,
            processing_user: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // get the title + url of the content in url
    pub async fn get_title(url: String) -> Result<Vec<(String, String)>> {
        let output = Command::new(YT_DLP_PATH)
            .arg("--cookies")
            .arg(COOKIES_PATH)
            .arg("--get-title")
            .arg(url.clone())
            .output()
            .await?;
        
        if output.status.success() {
            let title = String::from_utf8_lossy(&output.stdout);
            Ok(vec![(title.trim().to_string(), url.clone())])
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::ContentNotFound { msg: stderr.to_string() });
        }
    }

    // get the list of titles + urls in the playlist url
    pub async fn get_list(url: String) -> Result<Vec<(String, String)>> {

        let output = Command::new(YT_DLP_PATH)
            .arg("--cookies")
            .arg(COOKIES_PATH)
            .arg("--flat-playlist")
            .arg("--dump-single-json")
            .arg("--playlist-end")
            .arg(env::var("MAX_PLAYLIST_SIZE").unwrap_or("10".to_string()))
            .arg(url.clone())
            .output().await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed: Value = serde_json::from_str(&stdout).expect("Failed to parse JSON");

            if let Some(entries) = parsed["entries"].as_array() {
                // return a tuple of entry["title"] and entry["url"]
                let mut data = Vec::<(String, String)>::new();
                for entry in entries.iter() {
                    data.push(
                        (
                            entry["title"].as_str().map(String::from).unwrap_or("".to_string()),
                            entry["url"].as_str().map(String::from).unwrap_or("".to_string())
                        )
                    )
                }
                Ok(data)
            } else {
                return Err(Error::PlayListParseErr { msg: "no entries found in the playlist".to_string() });
            }
        } else {
            // Print the error message if the command fails
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::PlayListParseErr { msg: stderr.to_string() });
        }
    }

    pub async fn process_audio(&self, params: FMDownloadParams) -> Result<()> {

        let url = params.url.clone();
        let user_id = params.userid.clone().parse::<i32>().unwrap();
        // check if url is in db
        match sqlx::query(
            "SELECT * FROM files WHERE url = $1 AND user_id = $2"
        )
        .bind(url.clone())
        .bind(user_id)
        .fetch_all(&params.pool)
        .await {
            Ok(files) => {
                if files.len() > 0 {
                    let sender = self.get_sender_with_id(params.userid.clone()).await?;
                    sender.send(params.title.clone()).unwrap_or(0);
                    return Err(Error::DuplicateContent { msg: url.to_string() });
                }
            }
            Err(e) => {
                return Err(Error::DBError { source: e.to_string() });
            }
        }

        let sem_clone = self.semaphore.clone();
        let _permit = sem_clone.acquire().await.unwrap();

        self._process_audio(params).await?;

        Ok(())
    }

    pub async fn _process_audio(&self, params: FMDownloadParams) -> Result<()> {

        let url = params.url.clone();
        let title = params.title.clone();
        let userid: i32 = params.userid.clone().parse::<i32>().unwrap();
        let pool = params.pool.clone();
        let uuid = uuid::Uuid::new_v4().to_string();

        match FileManager::get_file_size(url.clone()).await {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }

        println!("processing-start: {}", url);
        // Ensure the output directory exists
        let converted_dir = "./converted";
        if !Path::new(converted_dir).exists() {
            tokio::fs::create_dir_all(converted_dir).await?;
        }

        let output_path = format!("{}/{}.ogg", converted_dir, uuid);

        let output = Command::new(YT_DLP_PATH)
            // Supply cookies
            .args(&["--cookies", COOKIES_PATH])
            // Point yt-dlp to custom FFmpeg
            .args(&["--ffmpeg-location", FFMPEG_PATH])
            // Use a sleep interval. If the range "0.5-1.5" is unsupported in your yt-dlp version,
            // replace the next line with:
            .args(&["--sleep-interval", "0.1", "--max-sleep-interval", "4"])
            // .args(&["--sleep-interval", "1"])
            // .args(&["--sleep-interval", "0.5", "--max-sleep-interval", "1.5"])
            // Download best available audio
            .args(&["-f", "bestaudio"])
            // Extract audio in one step
            .arg("-x")
            // Remove --audio-format to avoid yt-dlp renaming to .opus
            // (We'll let FFmpeg produce an Ogg container with Opus directly)
            // .args(&["--audio-format", "opus"])
            // Pass FFmpeg options to encode Opus in Ogg at 128 kbps with page duration
            .args(&[
                "--postprocessor-args",
                "ffmpeg:-c:a libopus -b:a 128k -page_duration 20000 -f ogg"
            ])
            // Explicitly name the final file .ogg
            .args(&["-o", &output_path])
            // Finally, the video URL
            .arg(url.clone())
            // Consider removing Stdio::null() if you want to see any errors or logs
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await?;
        
        println!("processing-end: {}", url);
        
        if !output.status.success() {
            return Err(Error::DownloadFailed { url: url.to_string() });
        }


        let change_file_name = Command::new("mv")
            .arg(format!("{}/{}.ogg.opus", converted_dir, uuid))
            .arg(format!("{}/{}.ogg", converted_dir, uuid))
            .output()
            .await?;

        // upload the file to s3
        let body = aws_sdk_s3::primitives::ByteStream::from_path(
            format!("{}/{}.ogg", converted_dir, uuid)
        ).await.unwrap();

        // if upload failed, also remove the files

        self.s3_client
            .put_object()
            .bucket("antaresmusicshare")
            .key(format!("{}.ogg", uuid))
            .body(body)
            .send()
            .await
            .map_err(
                // delete files if upload failed
                |e| {
                    tokio::fs::remove_file(format!("{}/{}.ogg", converted_dir, uuid));
                    Error::UploadFailed { msg: e.to_string() }
                }
            )?;

        println!("upload done: {}", url);
        // delete the files in convert
        tokio::fs::remove_file(format!("{}/{}.ogg", converted_dir, uuid)).await?;

        match sqlx::query(
            "
            INSERT INTO files (user_id, url, uuid, name)
            VALUES ($1, $2, $3, $4)
            ")
            .bind(userid)
            .bind(url.clone())
            .bind(uuid.clone())
            .bind(title)
            .execute(&pool)
            .await {
                Ok(_) => {
                }
                Err(e) => {
                    return Err(Error::DatabaseWriteError { msg: e.to_string() });
                }
            }
        
        let sender = self.get_sender_with_id(params.userid.clone()).await?;

        println!("task done: {}", url);
        sender.send("check".to_string()).unwrap_or(0);
        Ok(())
    }


    pub async fn is_live(url: String) -> Result<(bool)> {

        let output = Command::new(YT_DLP_PATH)
            .arg("--cookies")
            .arg(COOKIES_PATH)
            .arg("--print")
            .arg("%(is_live)s")
            .arg(url.clone())
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::InvalidURL {url: url.to_string()});
        }

        let result = String::from_utf8_lossy(&output.stdout).trim() == "True";
        if result {
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn get_file_size(youtube_url: String) -> Result<()> {

        dotenv().ok();

        // Path to the yt-dlp binary
        let yt_dlp_path = "./libs/yt-dlp";

        // Run yt-dlp to get the file size
        let output = Command::new(yt_dlp_path)
            .arg("--cookies")
            .arg(COOKIES_PATH)
            .arg("-f")
            .arg("bestaudio") // Specify best audio format
            .arg("--print")
            .arg("filesize") // Print the estimated file size
            .arg(youtube_url)
            .output()
            .await?;

        if !output.status.success() {
            eprintln!(
                "Failed to fetch file size. Error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Ok(());
        }

        let limit = env::var("MAX_FILE_SIZE")
            .unwrap_or("100000000".to_string())
            .parse::<u64>()
            .expect("MAX_FILE_SIZE must be a number");

        let size_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(size) = size_str.parse::<u64>() {

            if size > limit {
                return Err(Error::FileTooLarge { size, limit })
            }
        } 
        Ok(())
    }


    // funcationalties to send server side events upon completion of processing
    pub async fn get_sender_with_id(&self, id: String) -> Result<broadcast::Sender<String>> {
        let mut processing_user = self.processing_user.lock().await;
        let sender = processing_user.get(&id)
            .map(|f| f.clone()).unwrap();
        
        Ok(sender)
    }

    pub async fn add_sender_with_id(&self, id: String, sender: broadcast::Sender<String>) -> Result<()> {
        let mut processing_user = self.processing_user.lock().await;
        processing_user.insert(id, sender);
        Ok(())
    }

    pub async fn delete_file(&self, uuid: String) -> Result<()> {

        // delete the file from s3
        self.s3_client
            .delete_object()
            .bucket("antaresmusicshare")
            .key(format!("{}.ogg", uuid))
            .send()
            .await.map_err(
                |e| {
                    Error::S3Error { msg: e.to_string() }
                }
            )?;

        Ok(())
    }
}