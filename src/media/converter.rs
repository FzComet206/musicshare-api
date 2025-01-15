// a rust struct that converts mp3 files to webrtc compatible files using gstreamer
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::OnceLock;
use std::error::Error;

pub struct Converter;

impl Converter {
    pub fn init() -> Result<(), Box<dyn Error>> {
        gst::init()?;
        Ok(())
    }

    pub fn convert_to_opus(input: &str, output: &str) -> Result<(), Box<dyn Error>> {
        let pipeline = gst::parse_launch(&format!(
            "filesrc location={} ! decodebin ! audioconvert ! audioresample ! opusenc bitrate=64000 ! oggmux max-delay=20000000 ! filesink location={}",
            input, output
        ))?;

        pipeline.set_state(gst::State::Playing)?;
        let bus = pipeline.bus().unwrap();
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            match msg.view() {
                gst::MessageView::Eos(..) => break,
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Error from element {:?}: {}",
                        msg.src().map(|s| s.path_string()),
                        err.error()
                    );
                    eprintln!("Debugging information: {:?}", err.debug());
                    break;
                }
                _ => (),
            }
        }

        pipeline.set_state(gst::State::Null)?;
        Ok(())
    }
}