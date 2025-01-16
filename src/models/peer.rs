use webrtc::api::media_engine::MediaEngine;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::ice_transport::ice_candidate::{
    RTCIceCandidateInit,
    RTCIceCandidate,
};
use webrtc::ice_transport::ice_gatherer_state::RTCIceGathererState;

use tokio::sync::Mutex;
use tokio::sync::Notify;

use std::sync::Arc;

use crate::utils::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct PeerConnection{
    pub uuid: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub ice_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
    pub gathering_state: Arc<Notify>,
    pub active: Arc<Mutex<bool>>,
}

impl PeerConnection {

    pub async fn new(track: Arc<TrackLocalStaticSample>) -> Self {

        let mut m = MediaEngine::default();
        m.register_default_codecs();

        // Create a new API with the MediaEngine
        let api = APIBuilder::new().with_media_engine(m).build();
        // Define ICE servers
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                // urls: vec!["stun:stun.l.google.com:19302".to_string()],
                urls: vec![],
                ..Default::default()
            }],
            // ice_transport_policy: "all".to_string(),
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = api.new_peer_connection(config).await.unwrap();

        let rtp_sender = peer_connection.add_track(track.clone()).await.unwrap();

        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
            Result::<()>::Ok(())
        });


        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            peer_connection: Arc::new(peer_connection),
            ice_candidates: Arc::new(Mutex::new(Vec::new())),
            gathering_state: Arc::new(Notify::new()),
            active: Arc::new(Mutex::new(true)),
        }
    }


    pub async fn get_offer(& mut self) -> Result<String> {

        let pc = &mut self.peer_connection;

        // Use an Arc<Mutex> for `self.active` to make it thread-safe and `'static`
        let active = Arc::clone(&self.active); // Assume self.active is Arc<Mutex<bool>>

        pc.on_peer_connection_state_change(Box::new(move |state| {
            println!(
                "->> {:<12} Peer Connection State Change: {:?}",
                "PeerConnection", state
            );

            let active = Arc::clone(&active);

            Box::pin(async move {
                if state == webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::Disconnected {
                    // Set active to false
                    let mut active = active.lock().await;
                    *active = false;
                }
            })
        }));

        let offer = pc.create_offer(None).await?;
        pc.set_local_description(offer.clone()).await?;

        // Gather ICE candidates
        pc.on_ice_candidate(Box::new({
            let ice_candidates = Arc::clone(&self.ice_candidates);
            let notify = Arc::clone(&self.gathering_state);

            move |candidate| {
                let ice_candidates = Arc::clone(&ice_candidates);
                let notify = Arc::clone(&notify);

                Box::pin(async move {
                    if let Some(candidate) = candidate {
                        let mut candidates = ice_candidates.lock().await;
                        candidates.push(candidate);
                    } else {
                        // Notify waiters after gathering is complete
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        notify.notify_waiters();
                    }
                })
            }
        }));

        // Wait for local description and return SDP
        if let Some(local_description) = pc.local_description().await {
            Ok(local_description.sdp)
        } else {
            Err(Error::LocalDescriptionMissing)
        }
    }


    /// Sets an SDP answer
    pub async fn set_answer(&self, sdp: String) -> Result<()> {
        let remote_desc = RTCSessionDescription::answer(sdp)?;
        self.peer_connection.set_remote_description(remote_desc).await?;

        Ok(())
    }

    pub async fn get_ice(&self) -> Result<Vec<RTCIceCandidate>> {
        self.gathering_state.notified().await;
        let candidates = self.ice_candidates.lock().await.clone();

        Ok(candidates)
    }

    pub async fn add_ice(&self, candidate: RTCIceCandidateInit)-> Result<()> {
        self.peer_connection.add_ice_candidate(candidate).await?;
        Ok(())
    }

}