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

const OGG_PAGE_DURATION: Duration = Duration::from_millis(20);


#[derive(Clone, Debug)]
pub struct Broadcaster {
    pub audio_track: Arc<TrackLocalStaticSample>,
    is_broadcasting: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
}

impl Broadcaster {
    /// Initializes a new Broadcaster with a WebRTC PeerConnection
    pub async fn new() -> Result<Self> {

        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "broadcaster".to_owned(),
        ));


        Ok(Self {
            audio_track: track,
            is_broadcasting: Arc::new(AtomicBool::new(false)),
            is_paused: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn get_track(&mut self) -> Result<(Arc<TrackLocalStaticSample>)> {

        Ok(self.audio_track.clone())
    }


    pub async fn broadcast_audio_from_file(&self, file_path: &str) -> Result<()> {

        // upon function call, set the is_broadcasting flag to true
        self.is_broadcasting.store(true, Ordering::Release);

        let file_name = file_path.to_owned();
        let audio_track = self.audio_track.clone();

        let is_broadcasting = self.is_broadcasting.clone();
        let is_paused = self.is_paused.clone();

        let handle = tokio::spawn(async move {
            let file = File::open(file_name).unwrap();
            let reader = BufReader::new(file);
            let (mut ogg, _) = OggReader::new(reader, true).unwrap();
            
            let mut ticker = tokio::time::interval(OGG_PAGE_DURATION);

            let mut last_granule: u64 = 0;
            while let Ok((page_data, page_header)) = ogg.parse_next_page() {

                if is_broadcasting.load(Ordering::Acquire) == false {
                    break;
                }

                if is_paused.load(Ordering::Acquire) == true {
                    let _ = ticker.tick().await;
                    continue;
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
        });
        Ok(())
    }


    pub async fn stop(&self) {
        println!("Stopping broadcaster");
        self.is_broadcasting.store(false, Ordering::Release);
    }

    pub async fn pause(&self) {
        println!("Pausing broadcaster");
        self.is_paused.store(true, Ordering::Release);
    }

    pub async fn resume(&self) {
        println!("Resuming broadcaster");
        self.is_paused.store(false, Ordering::Release);
    }
}