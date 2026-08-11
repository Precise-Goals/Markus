//! GGUF file format parser — pure Rust, zero-copy mmap reads
//!
//! GGUF spec: https://github.com/ggerganov/ggml/blob/master/docs/gguf.md

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};

use crate::error::MarkusError;
use crate::Result;

const GGUF_MAGIC: u32 = 0x46554747; // "GGUF" in LE
const GGUF_VERSION_2: u32 = 2;
const GGUF_VERSION_3: u32 = 3;

/// Key-value value types as defined by the GGUF spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GgufValue {
    Uint8(u8),
    Int8(i8),
    Uint16(u16),
    Int16(i16),
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    Array(Vec<GgufValue>),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
}

impl GgufValue {
    pub fn as_str(&self) -> Option<&str> {
        match self { GgufValue::String(s) => Some(s), _ => None }
    }
    pub fn as_u32(&self) -> Option<u32> {
        match self { GgufValue::Uint32(v) => Some(*v), _ => None }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self { GgufValue::Uint64(v) => Some(*v), _ => None }
    }
    pub fn as_f32(&self) -> Option<f32> {
        match self { GgufValue::Float32(v) => Some(*v), _ => None }
    }
    pub fn as_u32_or_u64(&self) -> Option<u64> {
        match self {
            GgufValue::Uint32(v) => Some(*v as u64),
            GgufValue::Uint64(v) => Some(*v),
            _ => None,
        }
    }
}

/// Parsed metadata from GGUF header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufMeta {
    pub version: u32,
    pub tensor_count: u64,
    pub kv_count: u64,
    pub kv: HashMap<String, GgufValue>,
}

impl GgufMeta {
    pub fn architecture(&self) -> Option<&str> {
        self.kv.get("general.architecture").and_then(|v| v.as_str())
    }

    pub fn model_name(&self) -> Option<&str> {
        self.kv.get("general.name").and_then(|v| v.as_str())
    }

    pub fn context_length(&self) -> Option<u64> {
        let arch = self.architecture().unwrap_or("llama");
        let key = format!("{}.context_length", arch);
        self.kv.get(&key).and_then(|v| v.as_u32_or_u64())
    }

    pub fn embedding_length(&self) -> Option<u64> {
        let arch = self.architecture().unwrap_or("llama");
        let key = format!("{}.embedding_length", arch);
        self.kv.get(&key).and_then(|v| v.as_u32_or_u64())
    }

    pub fn head_count(&self) -> Option<u64> {
        let arch = self.architecture().unwrap_or("llama");
        let key = format!("{}.attention.head_count", arch);
        self.kv.get(&key).and_then(|v| v.as_u32_or_u64())
    }

    pub fn layer_count(&self) -> Option<u64> {
        let arch = self.architecture().unwrap_or("llama");
        let key = format!("{}.block_count", arch);
        self.kv.get(&key).and_then(|v| v.as_u32_or_u64())
    }

    pub fn vocab_size(&self) -> Option<u64> {
        self.kv.get("tokenizer.ggml.tokens")
            .and_then(|v| {
                if let GgufValue::Array(arr) = v { Some(arr.len() as u64) } else { None }
            })
    }

    pub fn quantization_type(&self) -> Option<u32> {
        self.kv.get("general.quantization_version").and_then(|v| v.as_u32())
    }

    /// Get the BOS token id
    pub fn bos_token_id(&self) -> Option<u32> {
        self.kv.get("tokenizer.ggml.bos_token_id").and_then(|v| v.as_u32())
    }

    /// Get the EOS token id
    pub fn eos_token_id(&self) -> Option<u32> {
        self.kv.get("tokenizer.ggml.eos_token_id").and_then(|v| v.as_u32())
    }

    /// Get all token strings for the vocabulary
    pub fn token_list(&self) -> Vec<String> {
        match self.kv.get("tokenizer.ggml.tokens") {
            Some(GgufValue::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => vec![],
        }
    }

    /// Get token scores (needed for SentencePiece models)
    pub fn token_scores(&self) -> Vec<f32> {
        match self.kv.get("tokenizer.ggml.scores") {
            Some(GgufValue::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_f32())
                .collect(),
            _ => vec![],
        }
    }

    /// Get token types (needed for special token detection)
    pub fn token_types(&self) -> Vec<u32> {
        match self.kv.get("tokenizer.ggml.token_type") {
            Some(GgufValue::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_u32())
                .collect(),
            _ => vec![],
        }
    }
}

/// GGUF Tensor descriptor (metadata only — data offset computed separately)
#[derive(Debug, Clone)]
pub struct GgufTensor {
    pub name: String,
    pub shape: Vec<u64>,
    pub dtype: u32,    // GGML dtype enum
    pub offset: u64,   // byte offset from tensor data section start
}

/// The main GGUF loader — parses the header and exposes metadata + tensor layout
pub struct GgufLoader {
    pub meta: GgufMeta,
    pub tensors: Vec<GgufTensor>,
    /// Byte offset where tensor data begins in the file
    pub data_offset: u64,
}

impl GgufLoader {
    /// Load and parse a GGUF file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)
            .map_err(|_| MarkusError::ModelNotFound { path: path.display().to_string() })?;
        let mut reader = std::io::BufReader::new(file);
        Self::parse(&mut reader)
    }

    fn parse<R: Read + Seek>(r: &mut R) -> Result<Self> {
        // Magic
        let magic = r.read_u32::<LittleEndian>()?;
        if magic != GGUF_MAGIC {
            return Err(MarkusError::GgufParse("Invalid GGUF magic number".into()));
        }

        // Version
        let version = r.read_u32::<LittleEndian>()?;
        if version != GGUF_VERSION_2 && version != GGUF_VERSION_3 {
            return Err(MarkusError::GgufParse(
                format!("Unsupported GGUF version: {}", version)
            ));
        }

        // Counts
        let tensor_count = r.read_u64::<LittleEndian>()?;
        let kv_count = r.read_u64::<LittleEndian>()?;

        // KV pairs
        let mut kv = HashMap::with_capacity(kv_count as usize);
        for _ in 0..kv_count {
            let key = read_gguf_string(r)?;
            let value = read_gguf_value(r)?;
            kv.insert(key, value);
        }

        let meta = GgufMeta { version, tensor_count, kv_count, kv };

        // Tensor metadata
        let mut tensors = Vec::with_capacity(tensor_count as usize);
        for _ in 0..tensor_count {
            let name = read_gguf_string(r)?;
            let ndim = r.read_u32::<LittleEndian>()?;
            let shape: Vec<u64> = (0..ndim)
                .map(|_| r.read_u64::<LittleEndian>())
                .collect::<std::io::Result<Vec<_>>>()?;
            let dtype = r.read_u32::<LittleEndian>()?;
            let offset = r.read_u64::<LittleEndian>()?;
            tensors.push(GgufTensor { name, shape, dtype, offset });
        }

        // Compute aligned data offset (GGUF aligns tensor data to 32 bytes)
        let current_pos = r.seek(SeekFrom::Current(0))?;
        let alignment = 32u64;
        let data_offset = (current_pos + alignment - 1) / alignment * alignment;

        Ok(Self { meta, tensors, data_offset })
    }

    /// Estimate VRAM/RAM needed to load this model
    pub fn estimated_memory_mb(&self) -> u64 {
        // Rough estimate: sum tensor sizes + 20% overhead for KV cache
        let tensor_bytes: u64 = self.tensors.iter().map(|t| {
            let elements: u64 = t.shape.iter().product();
            let bytes_per = ggml_type_size(t.dtype);
            elements * bytes_per / 8 // bits to bytes
        }).sum();
        (tensor_bytes / 1024 / 1024) * 12 / 10 // +20%
    }
}

/// Read a GGUF length-prefixed UTF-8 string
fn read_gguf_string<R: Read>(r: &mut R) -> Result<String> {
    let len = r.read_u64::<LittleEndian>()?;
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf)
        .map_err(|e| MarkusError::GgufParse(format!("Invalid UTF-8 string: {}", e)))
}

/// Read a typed GGUF value
fn read_gguf_value<R: Read>(r: &mut R) -> Result<GgufValue> {
    let vtype = r.read_u32::<LittleEndian>()?;
    Ok(match vtype {
        0  => GgufValue::Uint8(r.read_u8()?),
        1  => GgufValue::Int8(r.read_i8()?),
        2  => GgufValue::Uint16(r.read_u16::<LittleEndian>()?),
        3  => GgufValue::Int16(r.read_i16::<LittleEndian>()?),
        4  => GgufValue::Uint32(r.read_u32::<LittleEndian>()?),
        5  => GgufValue::Int32(r.read_i32::<LittleEndian>()?),
        6  => GgufValue::Float32(r.read_f32::<LittleEndian>()?),
        7  => GgufValue::Bool(r.read_u8()? != 0),
        8  => GgufValue::String(read_gguf_string(r)?),
        9  => {
            let elem_type = r.read_u32::<LittleEndian>()?;
            let count = r.read_u64::<LittleEndian>()?;
            let mut arr = Vec::with_capacity(count as usize);
            for _ in 0..count {
                arr.push(read_gguf_array_element(r, elem_type)?);
            }
            GgufValue::Array(arr)
        }
        10 => GgufValue::Uint64(r.read_u64::<LittleEndian>()?),
        11 => GgufValue::Int64(r.read_i64::<LittleEndian>()?),
        12 => GgufValue::Float64(r.read_f64::<LittleEndian>()?),
        t  => return Err(MarkusError::GgufParse(format!("Unknown GGUF value type: {}", t))),
    })
}

fn read_gguf_array_element<R: Read>(r: &mut R, elem_type: u32) -> Result<GgufValue> {
    Ok(match elem_type {
        0  => GgufValue::Uint8(r.read_u8()?),
        1  => GgufValue::Int8(r.read_i8()?),
        2  => GgufValue::Uint16(r.read_u16::<LittleEndian>()?),
        3  => GgufValue::Int16(r.read_i16::<LittleEndian>()?),
        4  => GgufValue::Uint32(r.read_u32::<LittleEndian>()?),
        5  => GgufValue::Int32(r.read_i32::<LittleEndian>()?),
        6  => GgufValue::Float32(r.read_f32::<LittleEndian>()?),
        7  => GgufValue::Bool(r.read_u8()? != 0),
        8  => GgufValue::String(read_gguf_string(r)?),
        10 => GgufValue::Uint64(r.read_u64::<LittleEndian>()?),
        11 => GgufValue::Int64(r.read_i64::<LittleEndian>()?),
        12 => GgufValue::Float64(r.read_f64::<LittleEndian>()?),
        t  => return Err(MarkusError::GgufParse(format!("Unknown GGUF array elem type: {}", t))),
    })
}

/// Returns bits per element for a GGML type
fn ggml_type_size(t: u32) -> u64 {
    match t {
        0  => 32, // F32
        1  => 16, // F16
        2  => 4,  // Q4_0 (approx)
        3  => 4,  // Q4_1
        6  => 5,  // Q5_0
        7  => 5,  // Q5_1
        8  => 8,  // Q8_0
        9  => 8,  // Q8_1
        10 => 2,  // Q2_K
        11 => 3,  // Q3_K
        12 => 4,  // Q4_K
        13 => 5,  // Q5_K
        14 => 6,  // Q6_K
        15 => 8,  // Q8_K
        16 => 8,  // I8
        17 => 16, // I16
        18 => 32, // I32
        _  => 32, // default f32
    }
}
