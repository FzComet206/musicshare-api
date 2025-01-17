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

const YT_DLP_PATH: &str = "./libs/yt-dlp";
const FFMPEG_PATH: &str = "./libs/ffmpeg";

pub struct FileManager;

impl FileManager {
    pub async fn init() -> Result<()> {
        // gst::init()?;
        Ok(())
    }

    pub async fn run_pipeline(url: String) -> Result<()> {
        // check if the url is a playlist with string manipulation
        // Self::is_live(url.clone()).await?;

        // determine if its playlist
        // determine if its stream

        Self::download_playlist(url.clone()).await?;
        Ok(())
    }

    async fn download_playlist(url: String) -> Result<()> {

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
                let semaphore = Arc::new(Semaphore::new(4));
                let mut tasks = FuturesUnordered::new();

                for url in urls {
                    // for each url in the playlist, initiate download pipeline
                    let sem_clone = semaphore.clone();

                    tasks.push(task::spawn(async move{

                        let _permit = sem_clone.acquire().await.unwrap();

                        println!("Downloading: {}", url);
                        Self::download_audio(url.clone(), "./output").await;

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

    async fn is_live(url: String) -> Result<()> {

        let output = Command::new(YT_DLP_PATH)
            .arg("--print")
            .arg("%(is_live)s")
            .arg(url.clone())
            .output()
            .await?;
        
        println!("checking");

        if !output.status.success() {
            return Err(Error::InvalidURL {url: url.to_string()});
        }

        let result = String::from_utf8_lossy(&output.stdout).trim() == "True";
        if result{
            return Err(Error::LiveStreamNotSupported {url: url.to_string()});
        }

        Ok(())
    }


    pub async fn download_audio(url: String, output_dir: &str) -> Result<()> {

        FileManager::get_file_size(url.clone()).await?;
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

        Ok(())
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


}