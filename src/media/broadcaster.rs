use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::error::Result;
use std::sync::Arc;
use webrtc::error::Error;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;

use std::time::Duration;
use webrtc::media::Sample;
use tokio::{fs::File, io::AsyncReadExt, time::interval};
use axum::body::Bytes;
use tokio::sync::Mutex;
use serde::Serialize;


#[derive(Clone, Debug)]
pub struct Broadcaster {
    peer_connection: Arc<RTCPeerConnection>,
    audio_track: Option<Arc<TrackLocalStaticSample>>,
    is_broadcasting: Arc<Mutex<bool>>, 
}

impl Broadcaster {
    /// Initializes a new Broadcaster with a WebRTC PeerConnection
    pub async fn new() -> Result<Self> {
        // Create a MediaEngine
        let mut m = MediaEngine::default();
        // Register default codecs
        m.register_default_codecs()?;

        // Create a new API with the MediaEngine
        let api = APIBuilder::new().with_media_engine(m).build();

        // Define ICE servers
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = api.new_peer_connection(config).await?;


        Ok(Self {
            audio_track: None,
            peer_connection: Arc::new(peer_connection),
            is_broadcasting: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn add_audio_track(&mut self, codec: &str) -> Result<()> {
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

        self.peer_connection.add_track(track.clone()).await?;
        // get self.peer connection's track

        self.audio_track = Some(track);
        Ok(())
    }

    pub async fn get_sdp_offer(&self) -> Result<String> {
        println!("->> {:<12} - get_sdp_offer", "Broadcaster");

        let offer = self.peer_connection.create_offer(None).await?;
        self.peer_connection.set_local_description(offer).await?;


        if let Some(local_description) = self.peer_connection.local_description().await {
            Ok(local_description.sdp)
        } else {
            Err(Error::new("Failed to get local description".to_string()))
        }
    }

    /// Sets an SDP answer
    pub async fn set_sdp_answer(&self, sdp: String) -> Result<()> {
        println!("->> {:<12} - set_sdp_answer", "Broadcaster");
        let remote_desc = RTCSessionDescription::answer(sdp)?;
        self.peer_connection.set_remote_description(remote_desc).await
    }

    pub async fn broadcast_audio_from_file(&self, file_path: &str) -> Result<()> {

        {
            let mut broadcasting = self.is_broadcasting.lock().await;
            *broadcasting = true;
        }
        // Open the file asynchronously
        let mut file = File::open(file_path).await.map_err(|e| {
            Error::new(format!("Failed to open file {}: {}", file_path, e))
        })?;
        
        // Buffer to read chunks of data
        let mut buffer = [0u8; 4096]; // Adjust size based on codec requirements
        let mut ticker = interval(Duration::from_millis(20)); // Example 20ms interval


        while let Ok(bytes_read) = file.read(&mut buffer).await {
            if bytes_read == 0 {
                break; // End of file
            }

            // Check if broadcasting is still active
            {
                let broadcasting = self.is_broadcasting.lock().await;
                if !*broadcasting {
                    break;
                }
            }

            // print bytes_read
            if let Some(track) = &self.audio_track {
                ticker.tick().await; // Wait for the next interval

                // Send the audio sample to the track
                track.write_sample(&Sample {
                    data: Bytes::copy_from_slice(&buffer[..bytes_read]), // Convert to Bytes
                    duration: Duration::from_millis(20), // Example duration
                    ..Default::default()
                }).await?;
            } else {
                return Err(Error::new("No audio track available for broadcasting".to_string()));
            }
        }

        Ok(())
    }

    pub async fn stop_broadcast(&self) {
        let mut broadcasting = self.is_broadcasting.lock().await;
        *broadcasting = false;
    }
}