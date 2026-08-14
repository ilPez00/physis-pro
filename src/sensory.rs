use serde::{Serialize, Deserialize};
use bytemuck::{Pod, Zeroable};
use std::time::SystemTime;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SignalType {
    AudioPCM = 0,
    Acceleration = 1,
    Gyroscope = 2,
    ThoughtCapture = 3,
    VisionFeature = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AuraFrameHeader {
    pub magic: [u8; 4], // 'AURA'
    pub _padding1: [u8; 4], // Align timestamp to 8
    pub timestamp: u64,
    pub signal_type: u8,
    pub _padding2: [u8; 3], // Align payload_len to 4
    pub payload_len: u32,
}

pub struct AuraFrame {
    pub header: AuraFrameHeader,
    pub payload: Vec<u8>,
}

impl AuraFrame {
    pub fn new(signal_type: SignalType, payload: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        Self {
            header: AuraFrameHeader {
                magic: *b"AURA",
                _padding1: [0; 4],
                timestamp,
                signal_type: signal_type as u8,
                _padding2: [0; 3],
                payload_len: payload.len() as u32,
            },
            payload,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = bytemuck::bytes_of(&self.header).to_vec();
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_SIZE: usize = std::mem::size_of::<AuraFrameHeader>();

    #[test]
    fn header_layout_is_wire_stable() {
        // The listener reads exactly this many bytes off the socket; a padding
        // or field change silently desyncs every producer.
        assert_eq!(HEADER_SIZE, 24);
    }

    #[test]
    fn new_frame_populates_header() {
        let frame = AuraFrame::new(SignalType::Gyroscope, vec![1, 2, 3]);
        assert_eq!(&frame.header.magic, b"AURA");
        assert_eq!(frame.header.signal_type, SignalType::Gyroscope as u8);
        assert_eq!(frame.header.payload_len, 3);
        assert!(frame.header.timestamp > 0);
    }

    #[test]
    fn to_bytes_roundtrips_through_listener_parse() {
        // Mirror the listener's decode: header bytes → bytemuck::from_bytes,
        // then payload_len bytes of payload.
        let payload = vec![9u8, 8, 7, 6];
        let frame = AuraFrame::new(SignalType::AudioPCM, payload.clone());
        let bytes = frame.to_bytes();
        assert_eq!(bytes.len(), HEADER_SIZE + payload.len());

        let parsed: AuraFrameHeader = *bytemuck::from_bytes(&bytes[..HEADER_SIZE]);
        assert_eq!(&parsed.magic, b"AURA");
        assert_eq!(parsed.signal_type, SignalType::AudioPCM as u8);
        assert_eq!(parsed.payload_len as usize, payload.len());
        assert_eq!(&bytes[HEADER_SIZE..], payload.as_slice());
    }

    #[test]
    fn empty_payload_frame_is_valid() {
        let frame = AuraFrame::new(SignalType::ThoughtCapture, Vec::new());
        assert_eq!(frame.header.payload_len, 0);
        assert_eq!(frame.to_bytes().len(), HEADER_SIZE);
    }
}

pub mod listener {
    use tokio::net::UnixListener;
    use tokio::io::AsyncReadExt;
    use crate::graph::IngestRing;
    use std::sync::Arc;

    pub async fn start_sensory_server(path: &str, ingest: Arc<IngestRing>) -> anyhow::Result<()> {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        
        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    let ingest_clone = ingest.clone();
                    tokio::spawn(async move {
                        let mut header_buf = [0u8; std::mem::size_of::<super::AuraFrameHeader>()];
                        while socket.read_exact(&mut header_buf).await.is_ok() {
                            let header: super::AuraFrameHeader = *bytemuck::from_bytes(&header_buf);
                            let mut payload = vec![0u8; header.payload_len as usize];
                            if socket.read_exact(&mut payload).await.is_ok() {
                                // Push raw payload to Physis Ingest Ring with signal type
                                ingest_clone.push(header.signal_type, payload);
                            }
                        }
                    });
                }
                Err(e) => log::error!("Sensory socket accept error: {}", e),
            }
        }
    }
}
