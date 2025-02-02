use webrtc::api::media_engine::{
    MediaEngine,
    MIME_TYPE_OPUS,
};
use webrtc::api::APIBuilder;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use std::sync::Arc;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use crate::utils::error::{Error, Result};

use std::time::Duration;
use std::io::BufReader;
use axum::body::Bytes;
use tokio::sync::Mutex;
use serde::Serialize;
use webrtc::rtp::extension::audio_level_extension::AudioLevelExtension;
use webrtc::media::{
    Sample,
    io::ogg_reader::OggReader,
};
use std::fs::File;
use std::path::Path;
use std::io::Write;
use tokio::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use std::collections::HashMap;
use tokio::sync::oneshot;

use crate::models::peer::PeerConnection;

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{config::Region, Client};

const OGG_PAGE_DURATION: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub enum BroadcasterCommand {
    Play { key: String },
    Pause,
    Stop,
    Attach {
        peer_id: String,
        reply: oneshot::Sender<()>,
    }
}

#[derive(Debug)]
pub enum BroadcasterEvent {
    End,
    TrackAdded,
}

#[derive(Clone, Debug)]
pub struct BroadcasterHandle {
    pub cmd_tx: mpsc::Sender<BroadcasterCommand>,
    pub event_rx: Arc<Mutex<mpsc::Receiver<BroadcasterEvent>>>
}

#[derive(Debug)]
pub struct Broadcaster {
    audio_track: Arc<TrackLocalStaticSample>,
    is_broadcasting: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<BroadcasterCommand>,
    event_tx: mpsc::Sender<BroadcasterEvent>,
    peer_connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    s3_client: Client,
    session_id: String,
}

impl Broadcaster {
    /// Initializes a new Broadcaster with a WebRTC PeerConnection
    pub async fn new(
        track: Arc<TrackLocalStaticSample>,
        cmd_rx: mpsc::Receiver<BroadcasterCommand>,
        event_tx: mpsc::Sender<BroadcasterEvent>,
        peer_connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
        session_id: String,

    ) -> Result<Self> {

        // s3 client for downloading single file only
        let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"));
        let region = region_provider.region().await.unwrap();
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        Ok(Self {
            audio_track: track,
            is_broadcasting: Arc::new(AtomicBool::new(false)),
            cmd_rx,
            event_tx,
            peer_connections,
            s3_client: client,
            session_id,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                BroadcasterCommand::Play { key } => {
                    if key.is_empty() {
                        continue;
                    }
                    match self.set_active_file(key.clone()).await {
                        Ok(file_path) => {
                            self.broadcast(&file_path).await?;
                        },
                        Err(e) => {
                            println!("Error setting active file: {:?}", e);
                        }
                    }
                }
                BroadcasterCommand::Pause => {
                }
                BroadcasterCommand::Stop => {
                    self.stop().await;
                }
                BroadcasterCommand::Attach { peer_id, reply } => {
                    println!("============= Attaching peer: {}", peer_id);

                    let pcs = self.peer_connections.lock().await;
                    match pcs.get(&peer_id) {
                        Some(pc) => {
                            match pc.add_track(self.audio_track.clone()).await {
                                Ok(_) => {
                                    match reply.send(()) {
                                        Ok(_) => {
                                            println!("Reply sent");
                                        },
                                        Err(_) => {
                                            println!("Cannot send reply");
                                        }
                                    }
                                },
                                Err(_) => {
                                    println!("Cannot add track");
                                },
                            }
                        },
                        None => {}
                    }
                    // let pc = pcs.get(&peer_id).unwrap();
                    // pc.add_track(self.audio_track.clone()).await;
                    // let _ = reply.send(());
                    println!("============= Attached peer: {}", peer_id);
                }
            }
        }
        Ok(())
    }
    
    pub async fn set_active_file(&self, key: String) -> Result<(String)> {

        // first check if file exists
        let session_id = self.session_id.clone();
        let session_dir = format!("./sessions/{}", session_id.clone());

        // Ensure the output directory exists
        if !Path::new(&session_dir).exists() {
            tokio::fs::create_dir_all(session_dir.clone()).await?;
        }

        // if there are any files left in session_dir, delete them
        let mut entries = match fs::read_dir(session_dir.clone()).await {
            Ok(entries) => entries,
            Err(e) => return Err(Error::ResetFileError { msg: e.to_string() }),
        };

        while let Some(entry) = entries.next_entry().await.transpose() {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(path).await?;
            }
        }
        
        let file_dir = format!("{}/{}.ogg", session_dir, key);

        let mut file = File::create(file_dir).map_err(|err| {
            Error::S3DownloadError { msg: "Failed to initialize file for s3 download".to_string() }
        })?;

        let mut object = self.s3_client
            .get_object()
            .bucket("antaresmusicshare")
            .key(format!("{}.ogg", key))
            .send()
            .await
            .map_err(
                |e| {
                    Error::S3DownloadError { msg: e.to_string() }
                }
            )?;
        
        // download the file to the session directory with file manager
        let mut byte_count = 0_usize;
        while let Some(bytes) = object.body.try_next().await.map_err(|err| {
            Error::S3DownloadError { msg: "Failed to read from s3 download streams".to_string() }
        })? {
            let bytes_len = bytes.len();
            file.write_all(&bytes).map_err(|err| {
                Error::S3DownloadError { msg: "Failed to write from s3 stream to local file".to_string() }
            })?;
            byte_count += bytes_len;
        }

        if byte_count == 0 {
            return Err(Error::S3DownloadError { msg: "Failed to download file".to_string() });
        }

        let file_path = format!("{}/{}.ogg", session_dir, key);

        Ok(file_path.to_string())
    }

    pub async fn broadcast(&self, 
        file_path: &str,
    ) -> Result<()> {

        // upon function call, set the is_broadcasting flag to true
        self.is_broadcasting.store(true, Ordering::Release);

        let file_name = file_path.to_owned();
        let audio_track = self.audio_track.clone();

        let is_broadcasting = self.is_broadcasting.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let file = File::open(file_name).unwrap();
            
            let reader = BufReader::new(file);
            let (mut ogg, _) = OggReader::new(reader, true).unwrap();
            
            let mut ticker = tokio::time::interval(OGG_PAGE_DURATION);

            let mut last_granule: u64 = 0;
            while let Ok((page_data, page_header)) = ogg.parse_next_page() {

                if is_broadcasting.load(Ordering::Acquire) == false {
                    // must return here so it doesnt send the End event
                    return;
                }

                let sample_count = page_header.granule_position - last_granule;
                last_granule = page_header.granule_position;
                let sample_duration = Duration::from_millis((sample_count * 1000) / 48000);
                // println!("Sample duration: {:?}", sample_duration);

                audio_track
                    .write_sample(&Sample {
                        data: page_data.freeze(),
                        duration: sample_duration,
                        ..Default::default()
                    }).await;

                let _ = ticker.tick().await;
            }

            event_tx.send(BroadcasterEvent::End).await;
        });
        Ok(())
    }

    pub async fn stop(&self) {
        self.is_broadcasting.store(false, Ordering::Release);
    }
}