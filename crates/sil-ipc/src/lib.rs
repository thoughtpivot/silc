//! Silc Shared Buffer ABI v1 — file-backed mmap slots + framed UDS control.

use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8; 4] = b"SILC";
pub const ABI_VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 128;
pub const DEFAULT_SLOT_COUNT: usize = 512;
// Feedback payloads are small (HTTP body ≤16KiB); keep slots lean so workers
// are not mmap'ing large files on every request.
pub const DEFAULT_PAYLOAD_CAPACITY: usize = 16 * 1024;
pub const PROTOCOL_VERSION: u16 = 1;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Empty = 0,
    Writing = 1,
    Ready = 2,
    Reading = 3,
    Retired = 4,
}

impl SlotState {
    pub fn from_u32(value: u32) -> Result<Self, String> {
        match value {
            0 => Ok(Self::Empty),
            1 => Ok(Self::Writing),
            2 => Ok(Self::Ready),
            3 => Ok(Self::Reading),
            4 => Ok(Self::Retired),
            other => Err(format!("invalid slot state {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub abi_version: u16,
    pub flags: u16,
    pub schema_id: u32,
    pub segment_id: u64,
    pub payload_offset: u32,
    pub payload_capacity: u32,
    pub payload_len: u32,
    pub seq: u64,
    pub state: SlotState,
    pub producer_id: u32,
    pub consumer_id: u32,
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..6].copy_from_slice(&self.abi_version.to_le_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_le_bytes());
        buf[8..12].copy_from_slice(&self.schema_id.to_le_bytes());
        buf[12..20].copy_from_slice(&self.segment_id.to_le_bytes());
        buf[20..24].copy_from_slice(&self.payload_offset.to_le_bytes());
        buf[24..28].copy_from_slice(&self.payload_capacity.to_le_bytes());
        buf[28..32].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[32..40].copy_from_slice(&self.seq.to_le_bytes());
        buf[40..44].copy_from_slice(&(self.state as u32).to_le_bytes());
        buf[44..48].copy_from_slice(&self.producer_id.to_le_bytes());
        buf[48..52].copy_from_slice(&self.consumer_id.to_le_bytes());
        buf
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_SIZE {
            return Err("header too short".into());
        }
        if &bytes[0..4] != MAGIC {
            return Err("bad SILC magic".into());
        }
        let abi_version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if abi_version != ABI_VERSION {
            return Err(format!("unsupported abi_version {abi_version}"));
        }
        Ok(Self {
            abi_version,
            flags: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            schema_id: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            segment_id: u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
            payload_offset: u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            payload_capacity: u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            payload_len: u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            seq: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
            state: SlotState::from_u32(u32::from_le_bytes(bytes[40..44].try_into().unwrap()))?,
            producer_id: u32::from_le_bytes(bytes[44..48].try_into().unwrap()),
            consumer_id: u32::from_le_bytes(bytes[48..52].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlFrame {
    Hello {
        worker_id: String,
        role: String,
        abi_version: u16,
        pid: u32,
    },
    Ready {
        worker_id: String,
    },
    Notify {
        request_id: String,
        segment_id: u64,
        offset: u32,
        len: u32,
        schema_id: u32,
        seq: u64,
        stage: String,
    },
    Ack {
        request_id: String,
        worker_id: String,
        segment_id: u64,
        seq: u64,
        #[serde(default)]
        result: Option<serde_json::Value>,
    },
    Error {
        request_id: Option<String>,
        code: String,
        message: String,
    },
    Shutdown {},
    Ingest {
        request_id: String,
        /// Optional for llm::complete payloads that only carry a prompt.
        #[serde(default)]
        author: String,
        /// Prompt text for llm::complete, or form body for text::score.
        #[serde(default)]
        text: String,
        /// Optional multi-chat session key persisted onto the chat record.
        #[serde(default)]
        session_id: String,
        /// Optional JSON / text application context for grounded llm::complete.
        #[serde(default)]
        context: String,
        /// Optional assistant identity/instructions for llm::complete.
        #[serde(default)]
        persona: String,
    },
    Response {
        request_id: String,
        ok: bool,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        score: Option<f64>,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        reply: Option<String>,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        error: Option<String>,
    },
}

pub fn write_frame<W: Write>(writer: &mut W, frame: &ControlFrame) -> Result<(), String> {
    let payload = serde_json::to_vec(frame).map_err(|e| e.to_string())?;
    if payload.len() > u32::MAX as usize {
        return Err("frame too large".into());
    }
    let len = payload.len() as u32;
    writer
        .write_all(&len.to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer
        .write_all(&PROTOCOL_VERSION.to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer.write_all(&payload).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<ControlFrame, String> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .map_err(|e| format!("read frame length: {e}"))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut ver_buf = [0u8; 2];
    reader
        .read_exact(&mut ver_buf)
        .map_err(|e| format!("read protocol version: {e}"))?;
    let version = u16::from_le_bytes(ver_buf);
    if version != PROTOCOL_VERSION {
        return Err(format!("unsupported protocol version {version}"));
    }
    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|e| format!("read frame payload: {e}"))?;
    serde_json::from_slice(&payload).map_err(|e| format!("decode frame: {e}"))
}

#[derive(Debug)]
pub struct SlotPool {
    pub root: PathBuf,
    pub schema_id: u32,
    pub slot_count: usize,
    pub payload_capacity: usize,
    maps: Vec<MmapMut>,
    free: Vec<usize>,
    next_seq: u64,
}

impl SlotPool {
    pub fn create(
        root: &Path,
        schema_id: u32,
        slot_count: usize,
        payload_capacity: usize,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| format!("create ipc dir: {e}"))?;
        let mut maps = Vec::with_capacity(slot_count);
        let mut free = Vec::with_capacity(slot_count);
        let slot_size = HEADER_SIZE + payload_capacity;
        for index in 0..slot_count {
            let path = root.join(format!("slot_{index:04}.sbuf"));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .map_err(|e| format!("open slot file: {e}"))?;
            file.set_len(slot_size as u64)
                .map_err(|e| format!("resize slot: {e}"))?;
            let mut map = unsafe {
                MmapOptions::new()
                    .len(slot_size)
                    .map_mut(&file)
                    .map_err(|e| format!("mmap slot: {e}"))?
            };
            let header = Header {
                abi_version: ABI_VERSION,
                flags: 0,
                schema_id,
                segment_id: index as u64,
                payload_offset: HEADER_SIZE as u32,
                payload_capacity: payload_capacity as u32,
                payload_len: 0,
                seq: 0,
                state: SlotState::Empty,
                producer_id: 0,
                consumer_id: 0,
            };
            map[..HEADER_SIZE].copy_from_slice(&header.encode());
            // MAP_SHARED updates are immediately visible to peer mappings.
            // Durability is not required for transient IPC slots.
            maps.push(map);
            free.push(index);
        }
        Ok(Self {
            root: root.to_path_buf(),
            schema_id,
            slot_count,
            payload_capacity,
            maps,
            free,
            next_seq: 1,
        })
    }

    pub fn open_existing(
        root: &Path,
        schema_id: u32,
        slot_count: usize,
        payload_capacity: usize,
    ) -> Result<Self, String> {
        let mut maps = Vec::with_capacity(slot_count);
        for index in 0..slot_count {
            let path = root.join(format!("slot_{index:04}.sbuf"));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|e| format!("open existing slot: {e}"))?;
            let map = unsafe {
                MmapOptions::new()
                    .len(HEADER_SIZE + payload_capacity)
                    .map_mut(&file)
                    .map_err(|e| format!("mmap existing slot: {e}"))?
            };
            let header = Header::decode(&map)?;
            if header.schema_id != schema_id {
                return Err("schema_id mismatch".into());
            }
            maps.push(map);
        }
        Ok(Self {
            root: root.to_path_buf(),
            schema_id,
            slot_count,
            payload_capacity,
            maps,
            free: (0..slot_count).collect(),
            next_seq: 1,
        })
    }

    pub fn acquire_write(&mut self, payload: &[u8]) -> Result<(usize, Header), String> {
        if payload.len() > self.payload_capacity {
            return Err("payload exceeds slot capacity".into());
        }
        let index = self
            .free
            .pop()
            .ok_or_else(|| "slot pool exhausted".to_string())?;
        let seq = self.next_seq;
        self.next_seq += 1;
        let header = Header {
            abi_version: ABI_VERSION,
            flags: 0,
            schema_id: self.schema_id,
            segment_id: index as u64,
            payload_offset: HEADER_SIZE as u32,
            payload_capacity: self.payload_capacity as u32,
            payload_len: payload.len() as u32,
            seq,
            state: SlotState::Ready,
            producer_id: 1,
            consumer_id: 0,
        };
        let map = &mut self.maps[index];
        map[..HEADER_SIZE].copy_from_slice(&header.encode());
        let start = HEADER_SIZE;
        let end = start + payload.len();
        map[start..end].copy_from_slice(payload);
        Ok((index, header))
    }

    pub fn read_payload(&self, index: usize) -> Result<(Header, Vec<u8>), String> {
        let map = self
            .maps
            .get(index)
            .ok_or_else(|| "bad slot index".to_string())?;
        let header = Header::decode(map)?;
        let start = header.payload_offset as usize;
        let end = start + header.payload_len as usize;
        if end > map.len() {
            return Err("payload out of bounds".into());
        }
        Ok((header, map[start..end].to_vec()))
    }

    pub fn update_payload(&mut self, index: usize, payload: &[u8]) -> Result<Header, String> {
        if payload.len() > self.payload_capacity {
            return Err("payload exceeds slot capacity".into());
        }
        let map = self
            .maps
            .get_mut(index)
            .ok_or_else(|| "bad slot index".to_string())?;
        let mut header = Header::decode(map)?;
        header.payload_len = payload.len() as u32;
        header.state = SlotState::Ready;
        map[..HEADER_SIZE].copy_from_slice(&header.encode());
        let start = HEADER_SIZE;
        map[start..start + payload.len()].copy_from_slice(payload);
        Ok(header)
    }

    pub fn release(&mut self, index: usize) -> Result<(), String> {
        let map = self
            .maps
            .get_mut(index)
            .ok_or_else(|| "bad slot index".to_string())?;
        let mut header = Header::decode(map)?;
        header.state = SlotState::Empty;
        header.payload_len = 0;
        map[..HEADER_SIZE].copy_from_slice(&header.encode());
        if !self.free.contains(&index) {
            self.free.push(index);
        }
        Ok(())
    }

    pub fn slot_path(&self, index: usize) -> PathBuf {
        self.root.join(format!("slot_{index:04}.sbuf"))
    }
}

pub fn map_slot_file(path: &Path) -> Result<(Header, Vec<u8>), String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let map = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|e| format!("mmap {}: {e}", path.display()))?
    };
    let header = Header::decode(&map)?;
    let start = header.payload_offset as usize;
    let end = start + header.payload_len as usize;
    if end > map.len() {
        return Err("payload out of bounds".into());
    }
    Ok((header, map[start..end].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "silc-ipc-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        path
    }

    #[test]
    fn header_roundtrip() {
        let header = Header {
            abi_version: ABI_VERSION,
            flags: 0,
            schema_id: 7,
            segment_id: 3,
            payload_offset: HEADER_SIZE as u32,
            payload_capacity: 1024,
            payload_len: 12,
            seq: 99,
            state: SlotState::Ready,
            producer_id: 1,
            consumer_id: 2,
        };
        let encoded = header.encode();
        let decoded = Header::decode(&encoded).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn slot_pool_write_read_release() {
        let root = temp_root();
        let mut pool = SlotPool::create(&root, 42, 4, 1024).unwrap();
        let payload = br#"{"hello":"world"}"#;
        let (index, header) = pool.acquire_write(payload).unwrap();
        assert_eq!(header.schema_id, 42);
        let (read_header, read_payload) = pool.read_payload(index).unwrap();
        assert_eq!(read_header.seq, header.seq);
        assert_eq!(read_payload, payload);
        pool.release(index).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn frame_roundtrip() {
        let frame = ControlFrame::Notify {
            request_id: "r1".into(),
            segment_id: 1,
            offset: HEADER_SIZE as u32,
            len: 10,
            schema_id: 1,
            seq: 2,
            stage: "python".into(),
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame).unwrap();
        let decoded = read_frame(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn ingest_defaults_missing_author_and_text() {
        // Chat clients historically sent only prompt/reply/model. Missing
        // author/text must deserialize to empty strings, not fail the frame.
        let json = br#"{"type":"INGEST","request_id":"r1","prompt":"hi","reply":"","model":""}"#;
        let frame: ControlFrame = serde_json::from_slice(json).expect("deserialize INGEST");
        match frame {
            ControlFrame::Ingest {
                request_id,
                author,
                text,
                session_id,
                context,
                persona,
            } => {
                assert_eq!(request_id, "r1");
                assert_eq!(author, "");
                assert_eq!(text, "");
                assert_eq!(session_id, "");
                assert_eq!(context, "");
                assert_eq!(persona, "");
            }
            other => panic!("expected Ingest, got {other:?}"),
        }
    }

    #[test]
    fn ingest_round_trips_context() {
        let frame = ControlFrame::Ingest {
            request_id: "r2".into(),
            author: "".into(),
            text: "how many widgets?".into(),
            session_id: "s1".into(),
            context: r#"[{"name":"widget","quantity":3}]"#.into(),
            persona: "You are the Inventory Assistant, built on silclm.".into(),
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame).unwrap();
        let decoded = read_frame(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, frame);
    }
}
