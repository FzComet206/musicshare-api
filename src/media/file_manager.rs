use crate::utils::error::{ Error, Result };
use std::path::Path;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task;
use futures::stream::{FuturesUnordered, StreamExt};
use sqlx::PgPool;
use dotenvy::dotenv;
use std::env;

const YT_DLP_PATH: &str = "./libs/yt-dlp";
const FFMPEG_PATH: &str = "./libs/ffmpeg";

pub struct FMDownloadParams{
    pub url: String,
    pub title: String,
    pub uuid: String,
    pub userid: u64,
    pub pool: PgPool,
}


#[derive(Clone, Debug)]
pub struct FileManager {
    pub semaphore: Arc<Semaphore>,
}

impl FileManager {
    pub async fn new() -> Result<(Self)> {

        dotenv().ok();
        let max_concurrent_downloads = env::var("MAX_CONCURRENT_DOWNLOADS")
            .unwrap_or("4".to_string())
            .parse::<usize>()
            .expect("MAX_CONCURRENT_DOWNLOADS must be a number");

        let semaphore = Arc::new(Semaphore::new(max_concurrent_downloads));

        Ok(Self {
            semaphore
        })
    }

    // get the title + url of the content in url
    pub async fn get_title(url: String) -> Result<Vec<(String, String)>> {
        let output = Command::new(YT_DLP_PATH)
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
            .arg("--flat-playlist")
            .arg("--dump-single-json")
            .arg("--playlist-end")
            .arg("20")
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
        let title = params.title.clone();
        let uuid = params.uuid.clone();
        let userid = params.userid.clone();
        let pool = params.pool.clone();

        // limit the number of concurrent downloads
        let sem_clone = self.semaphore.clone();
        let _permit = sem_clone.acquire().await.unwrap();

        FileManager::get_file_size(url.clone()).await?;
        let output_dir = "./output";
        let converted_dir = "./converted";

        // Ensure the output directory exists
        if !Path::new(output_dir).exists() {
            tokio::fs::create_dir_all(output_dir).await?;
        }

        if !Path::new(converted_dir).exists() {
            tokio::fs::create_dir_all(converted_dir).await?;
        }

        let mut uuid = uuid::Uuid::new_v4().to_string();

        // Construct the command to download audio
        let output = Command::new(YT_DLP_PATH)
            .arg("-f")
            .arg("bestaudio") // Best available audio format
            .arg("--extract-audio") // Extract audio only
            .arg("--audio-format")
            .arg("mp3") // Convert audio to OGG format
            .arg("--output")
            // .arg(format!("{}/%(title)s.%(ext)s", output_dir)) // Set output file format
            .arg(format!("{}/{}", output_dir, uuid)) // Set output file format
            .arg(url.clone())
            .output()
            .await?;
        
        if output.status.success() {
            println!("Audio downloaded successfully!");
        } else {
            eprintln!("Failed to download audio. Status: {:?}", output.status);
        }

        let status = Command::new(FFMPEG_PATH)
            .arg("-y")
            .arg("-i")
            .arg(format!("{}/{}.mp3", output_dir, uuid)) // Input file
            .arg("-c:a")
            .arg("libopus") // Use Opus codec
            .arg("-page_duration")
            .arg("20000") // Set page duration
            .arg("-vn") // Disable video
            .arg(format!("{}/{}.ogg", converted_dir, uuid)) // Output file
            .status()
            .await?;
    
        // store audio title and uuid to database

        match sqlx::query(
            "
            INSERT INTO files (user_id, url, uuid, name)
            VALUES ($1, $2, $3, $4)
            ")
            .bind(userid as i64)
            .bind(url)
            .bind(uuid)
            .bind(title)
            .execute(&pool)
            .await {
                Ok(_) => {
                    println!("Audio metadata stored successfully!");
                }
                Err(e) => {
                    eprintln!("Failed to store audio metadata. Error: {:?}", e);
                }
            }

        Ok(())
    }


    pub async fn is_live(url: String) -> Result<(bool)> {

        let output = Command::new(YT_DLP_PATH)
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
    // Path to the yt-dlp binary
        let yt_dlp_path = "./libs/yt-dlp";

        // Run yt-dlp to get the file size
        let output = Command::new(yt_dlp_path)
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

        let limit = 200_000_000;
        let size_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(size) = size_str.parse::<u64>() {
            println!("File size: {}", size);

            if size > limit {
                return Err(Error::FileTooLarge { size, limit })
            }
        } 
        Ok(())
    }

    async fn download_playlist(&self, url: String) -> Result<()> {

        let output = Command::new(YT_DLP_PATH)
            .arg("--flat-playlist")
            .arg("--dump-single-json")
            .arg("--playlist-end")
            .arg("20")
            .arg(url.clone())
            .output().await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed: Value = serde_json::from_str(&stdout).expect("Failed to parse JSON");

            if let Some(entries) = parsed["entries"].as_array() {
                let urls: Vec<_> = entries
                    .iter()
                    .filter_map(|entry| {
                        entry["url"].as_str().map(String::from)
                    })
                    .collect();
                
                // task pool size
                let mut tasks = FuturesUnordered::new();

                for url in urls {
                    // for each url in the playlist, initiate download pipeline
                    let sem_clone = self.semaphore.clone();

                    tasks.push(task::spawn(async move{

                        let _permit = sem_clone.acquire().await.unwrap();
                        println!("Starting task: {}", url);
                        // Self::download_audio(url.clone()).await;

                    }));
                }

            } else {
                return Err(Error::PlayListParseErr { msg: "no entries found in the playlist".to_string() });
            }
        } else {
            // Print the error message if the command fails
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::PlayListParseErr { msg: stderr.to_string() });
        }
        Ok(())
    }
}