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
use webrtc::error::{Error, Result};
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;

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
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use std::collections::HashMap;
use crate::models::peer::PeerConnection;
use tokio::sync::oneshot;

const OGG_PAGE_DURATION: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub enum BroadcasterCommand {
    Play { file_path: String },
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
}

impl Broadcaster {
    /// Initializes a new Broadcaster with a WebRTC PeerConnection
    pub async fn new(
        track: Arc<TrackLocalStaticSample>,
        cmd_rx: mpsc::Receiver<BroadcasterCommand>,
        event_tx: mpsc::Sender<BroadcasterEvent>,
        peer_connections: Arc<Mutex<HashMap<String, PeerConnection>>>,

    ) -> Result<Self> {


        Ok(Self {
            audio_track: track,
            is_broadcasting: Arc::new(AtomicBool::new(false)),
            cmd_rx,
            event_tx,
            peer_connections,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                BroadcasterCommand::Play { file_path } => {
                    self.broadcast(&file_path).await?;
                    println!("Playing file: {}", file_path);
                }
                BroadcasterCommand::Pause => {
                    println!("Pausing broadcaster");
                }
                BroadcasterCommand::Stop => {
                    self.stop().await;
                    println!("Stopping broadcaster");
                }
                BroadcasterCommand::Attach { peer_id, reply } => {
                    let pcs = self.peer_connections.lock().await;
                    let pc = pcs.get(&peer_id).unwrap();
                    pc.add_track(self.audio_track.clone()).await;
                    let _ = reply.send(());
                }
            }
        }
        Ok(())
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
                    println!("Stopping broadcaster");
                    break;
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
        println!("Stopping broadcaster");
        self.is_broadcasting.store(false, Ordering::Release);
    }
}