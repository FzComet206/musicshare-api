// a rust struct that converts mp3 files to webrtc compatible files using gstreamer

use crate::utils::error::{ Error, Result };

use tokio::process::Command;
use std::path::Path;

pub struct FileManager;

impl FileManager {
    pub async fn init() -> Result<()> {
        // gst::init()?;
        Ok(())
    }

    pub async fn download_audio(url: &str, output_dir: &str) -> Result<()> {

         let yt_dlp_path = "./libs/yt-dlp";
         let ffmpeg_path = "./libs/ffmpeg";

         FileManager::get_file_size(url).await?;

        // Ensure the output directory exists
        if !Path::new(output_dir).exists() {
            tokio::fs::create_dir_all(output_dir).await?;
        }

        let mut uuid = uuid::Uuid::new_v4().to_string();

        // Construct the command to download audio
        let output = Command::new(yt_dlp_path)
            .arg("-f")
            .arg("bestaudio") // Best available audio format
            .arg("--extract-audio") // Extract audio only
            .arg("--audio-format")
            .arg("mp3") // Convert audio to OGG format
            .arg("--output")
            // .arg(format!("{}/%(title)s.%(ext)s", output_dir)) // Set output file format
            .arg(format!("{}/test", output_dir)) // Set output file format
            .arg(url)
            .output()
            .await?;
        
        if output.status.success() {
            println!("Audio downloaded successfully!");
        } else {
            eprintln!("Failed to download audio. Status: {:?}", output.status);
        }

        let filename = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("Filename: {}", filename);

        let status = Command::new(ffmpeg_path)
            .arg("-y")
            .arg("-i")
            .arg(format!("{}/test.mp3", output_dir)) // Input file
            .arg("-c:a")
            .arg("libopus") // Use Opus codec
            .arg("-page_duration")
            .arg("20000") // Set page duration
            .arg("-vn") // Disable video
            .arg(format!("{}/converted.ogg", output_dir)) // Output file
            .status()
            .await?;

        Ok(())
    }

    pub async fn get_file_size(youtube_url: &str) -> Result<()> {
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