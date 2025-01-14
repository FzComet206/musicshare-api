use webrtc::api::media_engine::MediaEngine;
use webrtc::api::media_engine::MIME_TYPE_VP8;
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
use webrtc::media::Sample;
use tokio::{fs::File, io::AsyncReadExt, time::interval};
use axum::body::Bytes;
use tokio::sync::Mutex;
use serde::Serialize;
use webrtc::rtp::extension::audio_level_extension::AudioLevelExtension;


#[derive(Clone, Debug)]
pub struct Broadcaster {
    pub audio_track: Option<Arc<TrackLocalStaticSample>>,
    // pub is_broadcasting: Arc<Mutex<bool>>, 
}

impl Broadcaster {
    /// Initializes a new Broadcaster with a WebRTC PeerConnection
    pub async fn new() -> Result<Self> {
        // Create a MediaEngine
        Ok(Self {
            audio_track: None,
            // is_broadcasting: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn add_audio_track(&mut self, codec: &str, peer_connection: Arc<RTCPeerConnection>) -> Result<()> {
        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: codec.to_string(),
                clock_rate: 48000,
                channels: 2,
                ..Default::default()
            },
            "audio".to_string(),
            "broadcaster".to_string(),
        ));

        peer_connection.add_track(track.clone()).await?;
        // get self.peer connection's track

        self.audio_track = Some(track);

        Ok(())
    }


    pub async fn broadcast_audio_from_file(&self, file_path: &str) -> Result<()> {

        // Open the file asynchronously
        // let mut file = File::open(file_path).await.map_err(|e| {
            // Error::new(format!("Failed to open file {}: {}", file_path, e))
        // })?;

        let mut file = tokio::fs::File::open(file_path).await.unwrap();
        
        // Buffer to read chunks of data
        let samples_per_channel = 265; // 20ms of audio at 48kHz
        let channels = 2; // Stereo
        let bytes_per_sample = 2; // 16-bit PCM
        let frame_size = samples_per_channel * channels * bytes_per_sample;
        let mut buffer = vec![0; frame_size];

        // let mut buffer = [0u8; frame_size]; // Adjust size based on codec requirements
        let mut ticker = interval(Duration::from_millis(5)); // Example 20ms interval

        while let Ok(bytes_read) = file.read(&mut buffer).await {
            println!("Bytes read: {}", bytes_read);
            if bytes_read == 0 {
                break; // End of file
            }

            // print bytes_read
            if let Some(track) = &self.audio_track {
                ticker.tick().await; // Wait for the next interval

                // print out the buffer
                // Send the audio sample to the track
                track
                .sample_writer()
                .write_sample(&Sample {
                    data: Bytes::copy_from_slice(&buffer[..bytes_read]), // Convert to Bytes
                    // data: Bytes::from_static(&[0;1024]),
                    duration: Duration::from_millis(5), // Example duration
                    ..Default::default()
                }).await?;


            } else {
                return Err(Error::new("No audio track available for broadcasting".to_string()));
            }
        }

        Ok(())
    }

    // pub async fn stop_broadcast(&self) {
        // let mut broadcasting = self.is_broadcasting.lock().await;
        // *broadcasting = false;
    // }
}