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

const OGG_PAGE_DURATION: Duration = Duration::from_millis(20);


#[derive(Clone, Debug)]
pub struct Broadcaster {
    pub audio_track: Arc<TrackLocalStaticSample>,
    // pub is_broadcasting: Arc<Mutex<bool>>, 
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
            // is_broadcasting: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn get_track(&mut self) -> Result<(Arc<TrackLocalStaticSample>)> {

        Ok(self.audio_track.clone())
    }


    pub async fn broadcast_audio_from_file(&self, file_path: &str) -> Result<()> {

        let file_name = file_path.to_owned();
        let audio_track = self.audio_track.clone();

        tokio::spawn(async move {
            let file = File::open(file_name).unwrap();
            let reader = BufReader::new(file);
            let (mut ogg, _) = OggReader::new(reader, true).unwrap();
            
            let mut ticker = tokio::time::interval(OGG_PAGE_DURATION);

            let mut last_granule: u64 = 0;
            while let Ok((page_data, page_header)) = ogg.parse_next_page() {

                let sample_count = page_header.granule_position - last_granule;
                last_granule = page_header.granule_position;
                let sample_duration = Duration::from_millis((sample_count * 1000) / 48000);
                println!("Sample duration: {:?}", sample_duration);

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

    // pub async fn stop_broadcast(&self) {
        // let mut broadcasting = self.is_broadcasting.lock().await;
        // *broadcasting = false;
    // }
}