use std::{
    borrow::Cow,
    io::Read,
    sync::{Arc, Mutex},
};

use bitflags::bitflags;

use bytes::Bytes;
use libflate::gzip;

use crate::runetek5::io::packet::Packet;

#[derive(Debug)]
enum Js5CompressionType {
    None,
    Bzip2,
    Gzip,
    Lzma,
}

impl TryFrom<u8> for Js5CompressionType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Js5CompressionType::None),
            1 => Ok(Js5CompressionType::Bzip2),
            2 => Ok(Js5CompressionType::Gzip),
            3 => Ok(Js5CompressionType::Lzma),
            _ => Err("Invalid compression type"),
        }
    }
}

const BZIP2_HEADER: &[u8] = b"BZh1";

pub fn decompress(mut data: &[u8]) -> Vec<u8> {
    use bytes::Buf;
    let compression_type: Js5CompressionType = data.g1().try_into().unwrap();
    let compressed_size = data.g4();
    // println!("{:?}, {:?}", compression_type, compressed_size);
    match compression_type {
        Js5CompressionType::None => {
            let mut decompressed = Vec::with_capacity(compressed_size as usize);
            decompressed.extend_from_slice(&data[..compressed_size as usize]);
            decompressed
        }
        Js5CompressionType::Bzip2 => {
            let decompressed_size = data.g4();
            let buf_with_header = Buf::chain(BZIP2_HEADER, data);
            let mut decoder = bzip2_rs::DecoderReader::new(buf_with_header.reader());
            let mut decompressed = Vec::with_capacity(decompressed_size as usize);
            decoder.read_to_end(&mut decompressed).unwrap();
            decompressed
        }
        Js5CompressionType::Gzip => {
            let decompressed_size = data.g4();
            let mut decoder = gzip::Decoder::new(data.reader()).unwrap();
            let mut decompressed = Vec::with_capacity(decompressed_size as usize);
            decoder.read_to_end(&mut decompressed).unwrap();
            decompressed
        }
        Js5CompressionType::Lzma => {
            unimplemented!();
        }
    }
}

const WHIRLPOOL_HASH_SIZE: usize = 64;
type WhirlpoolHash = [u8; WHIRLPOOL_HASH_SIZE];

const MD5_HASH_SIZE: usize = 16;
type Md5Hash = [u8; MD5_HASH_SIZE];

#[derive(PartialEq, PartialOrd, Debug, Clone, Copy)]
pub enum Js5IndexProtocol {
    Original = 5,
    Versioned = 6,
    Smart = 7,
}

impl TryFrom<u8> for Js5IndexProtocol {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            5 => Ok(Js5IndexProtocol::Original),
            6 => Ok(Js5IndexProtocol::Versioned),
            7 => Ok(Js5IndexProtocol::Smart),
            _ => Err("Invalid index protocol"),
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Js5IndexFlags: u8 {
        const NAMES = 1 << 0;
        const WHIRLPOOL_HASHES = 1 << 1;
        const GROUP_DATA_SIZES = 1 << 2;
        const UNCOMPRESSED_CHECKSUMS = 1 << 3;
        /// Custom flag for deduplication of files
        const MD5_HASHES = 1 << 7;
    }
}

#[derive(Debug, Clone)]
pub struct Js5Index {
    pub crc: u32,
    pub protocol: Js5IndexProtocol,
    pub version: u32,
    pub has_names: bool,
    pub has_whirlpool_hashes: bool,
    pub has_group_data_sizes: bool,
    pub has_uncompressed_checksums: bool,
    pub has_md5_hashes: bool,
    pub group_count: u32,
    pub group_capacity: u32,
    pub group_ids: Vec<u32>,
    pub group_name_hashes: Option<Vec<i32>>,
    pub group_checksums: Vec<u32>,
    pub group_uncompressed_checksums: Option<Vec<u32>>,
    pub group_whirlpool_hashes: Option<Vec<WhirlpoolHash>>,
    pub group_data_sizes: Option<Vec<u32>>,
    pub group_uncompressed_data_sizes: Option<Vec<u32>>,
    pub group_versions: Vec<u32>,
    pub group_file_counts: Vec<u32>,
    pub group_file_capacities: Vec<u32>,
    pub group_file_ids: Vec<Option<Vec<u32>>>,
    pub group_file_name_hashes: Option<Vec<Vec<i32>>>,
    pub group_md5_hashes: Option<Vec<Md5Hash>>,
}

impl Js5Index {
    pub const ARCHIVE_ID: u8 = 255;

    pub fn decode(data: &[u8], expected_crc: Option<u32>) -> Js5Index {
        let crc = crc32fast::hash(data);
        if let Some(expected_crc) = expected_crc {
            assert_eq!(crc, expected_crc);
        }

        let mut buffer = Bytes::from(decompress(data));
        let protocol: Js5IndexProtocol = buffer.g1().try_into().unwrap();
        let mut version = 0;
        if protocol >= Js5IndexProtocol::Versioned {
            version = buffer.g4();
        }
        let flags = Js5IndexFlags::from_bits_retain(buffer.g1());
        let has_names = flags.contains(Js5IndexFlags::NAMES);
        let has_whirlpool_hashes = flags.contains(Js5IndexFlags::WHIRLPOOL_HASHES);
        let has_group_data_sizes = flags.contains(Js5IndexFlags::GROUP_DATA_SIZES);
        let has_uncompressed_checksums = flags.contains(Js5IndexFlags::UNCOMPRESSED_CHECKSUMS);
        let has_md5_hashes = flags.contains(Js5IndexFlags::MD5_HASHES);

        let read = if protocol == Js5IndexProtocol::Smart {
            |buffer: &mut Bytes| buffer.get_smart_2_or_4()
        } else {
            |buffer: &mut Bytes| buffer.g2() as u32
        };

        let group_count = read(&mut buffer);

        let mut group_ids = vec![0; group_count as usize];

        let mut last_group_id = 0;
        for i in 0..group_count {
            last_group_id += read(&mut buffer);
            group_ids[i as usize] = last_group_id;
        }

        let group_capacity = if group_count == 0 {
            0
        } else {
            last_group_id + 1
        };

        let mut group_name_hashes: Option<Vec<i32>> = None;
        if has_names {
            let mut hashes = vec![-1; group_capacity as usize];
            for i in 0..group_count {
                hashes[group_ids[i as usize] as usize] = buffer.g4s();
            }
            group_name_hashes = Some(hashes);
        }

        let mut group_checksums = vec![0; group_capacity as usize];
        for i in 0..group_count {
            group_checksums[group_ids[i as usize] as usize] = buffer.g4();
        }

        let mut group_uncompressed_checksums: Option<Vec<u32>> = None;
        if has_uncompressed_checksums {
            let mut checksums = vec![0; group_capacity as usize];
            for i in 0..group_count {
                checksums[group_ids[i as usize] as usize] = buffer.g4();
            }
            group_uncompressed_checksums = Some(checksums);
        }

        let mut group_whirlpool_hashes: Option<Vec<WhirlpoolHash>> = None;
        if has_whirlpool_hashes {
            let mut hashes = vec![[0; WHIRLPOOL_HASH_SIZE]; group_capacity as usize];
            for i in 0..group_count {
                buffer.get_array(&mut hashes[group_ids[i as usize] as usize]);
            }
            group_whirlpool_hashes = Some(hashes);
        }

        let mut group_data_sizes: Option<Vec<u32>> = None;
        let mut group_uncompressed_data_sizes: Option<Vec<u32>> = None;
        if has_group_data_sizes {
            let mut lengths = vec![0; group_capacity as usize];
            let mut uncompressed_lengths = vec![0; group_capacity as usize];
            for i in 0..group_count {
                let group_id = group_ids[i as usize] as usize;
                lengths[group_id] = buffer.g4();
                uncompressed_lengths[group_id] = buffer.g4();
            }
            group_data_sizes = Some(lengths);
            group_uncompressed_data_sizes = Some(uncompressed_lengths);
        }

        let mut group_versions = vec![0; group_capacity as usize];
        for i in 0..group_count {
            group_versions[group_ids[i as usize] as usize] = buffer.g4();
        }

        let mut group_file_counts = vec![0; group_capacity as usize];
        for i in 0..group_count {
            group_file_counts[group_ids[i as usize] as usize] = read(&mut buffer);
        }

        let mut group_file_capacities = vec![0; group_capacity as usize];

        let mut group_file_ids: Vec<Option<Vec<u32>>> = vec![None; group_capacity as usize];
        for i in 0..group_count {
            let group_id = group_ids[i as usize] as usize;
            let file_count = group_file_counts[group_id];

            let mut file_ids: Vec<u32> = vec![0; file_count as usize];

            let mut last_file_id = 0;
            for j in 0..file_count {
                last_file_id += read(&mut buffer);
                file_ids[j as usize] = last_file_id;
            }

            let file_capacity = if file_count == 0 { 0 } else { last_file_id + 1 };

            group_file_capacities[group_id] = file_capacity;

            if file_count != file_capacity {
                group_file_ids[group_id as usize] = Some(file_ids);
            }
        }

        let mut group_file_name_hashes: Option<Vec<Vec<i32>>> = None;
        if has_names {
            let mut file_name_hashes = vec![vec![-1; 0]; group_capacity as usize];
            for i in 0..group_count {
                let group_id = group_ids[i as usize] as usize;
                let file_count = group_file_counts[group_id];
                let mut hashes = vec![-1; file_count as usize];
                for j in 0..file_count {
                    hashes[j as usize] = buffer.g4s();
                }
                file_name_hashes[group_id] = hashes;
            }
            group_file_name_hashes = Some(file_name_hashes);
        }

        let mut group_md5_hashes: Option<Vec<Md5Hash>> = None;
        if has_md5_hashes {
            let mut hashes = vec![[0; MD5_HASH_SIZE]; group_capacity as usize];
            for i in 0..group_count {
                buffer.get_array(&mut hashes[group_ids[i as usize] as usize]);
            }
            group_md5_hashes = Some(hashes);
        }

        Js5Index {
            crc,
            protocol,
            version,
            has_names,
            has_whirlpool_hashes,
            has_group_data_sizes,
            has_uncompressed_checksums,
            has_md5_hashes,
            group_count,
            group_capacity,
            group_ids,
            group_name_hashes,
            group_checksums,
            group_uncompressed_checksums,
            group_whirlpool_hashes,
            group_data_sizes,
            group_uncompressed_data_sizes,
            group_versions,
            group_file_counts,
            group_file_capacities,
            group_file_ids,
            group_file_name_hashes,
            group_md5_hashes,
        }
    }

    pub fn clear_data_sizes(&mut self) {
        self.group_data_sizes = None;
        self.group_uncompressed_data_sizes = None;
    }

    pub fn get_group_version(&self, group_id: u32) -> u32 {
        self.group_versions[group_id as usize]
    }

    pub fn get_group_crc(&self, group_id: u32) -> u32 {
        self.group_checksums[group_id as usize]
    }

    pub fn get_file_count(&self, group_id: u32) -> u32 {
        self.group_file_counts[group_id as usize]
    }

    pub fn get_file_capacity(&self, group_id: u32) -> u32 {
        self.group_file_capacities[group_id as usize]
    }

    pub fn get_file_ids(&self, group_id: u32) -> Option<&Vec<u32>> {
        self.group_file_ids[group_id as usize].as_ref()
    }
}

pub trait Js5ResourceProvider {
    fn fetch_index(&self) -> Option<Arc<Js5Index>>;

    fn fetch_group(&self, group_id: u32) -> Option<Bytes>;
}

pub struct Js5GroupData {
    packed: Option<Bytes>,
    unpacked: Option<Vec<Option<Bytes>>>,
}

pub struct Js5 {
    pub provider: Arc<dyn Js5ResourceProvider + Send + Sync>,
    pub index: Arc<Js5Index>,
    discard_packed: bool,
    discard_unpacked: bool,
    groups: Vec<Mutex<Js5GroupData>>,
}

impl Js5 {
    pub fn new(
        provider: Arc<dyn Js5ResourceProvider + Send + Sync>,
        index: Arc<Js5Index>,
        discard_packed: bool,
        discard_unpacked: bool,
    ) -> Self {
        let groups = (0..index.group_capacity)
            .map(|_| {
                Mutex::new(Js5GroupData {
                    packed: None,
                    unpacked: None,
                })
            })
            .collect::<Vec<_>>();
        Self {
            provider,
            index,
            discard_packed,
            discard_unpacked,
            groups,
        }
    }

    pub fn get_version(&self) -> u32 {
        self.index.version
    }

    pub fn get_crc(&self) -> u32 {
        self.index.crc
    }

    pub fn get_group_count(&self) -> u32 {
        self.index.group_count
    }

    // Maybe return Option<u32> when group count is 0
    pub fn get_last_group_id(&self) -> u32 {
        self.index.group_capacity - 1
    }

    pub fn get_file_count(&self, group_id: u32) -> u32 {
        self.index.get_file_count(group_id)
    }

    pub fn get_file_capacity(&self, group_id: u32) -> u32 {
        self.index.get_file_capacity(group_id)
    }

    pub fn get_file_ids(&self, group_id: u32) -> Option<Cow<Vec<u32>>> {
        if !self.is_group_valid(group_id) {
            return None;
        }
        if let Some(file_ids) = self.index.get_file_ids(group_id) {
            Some(Cow::Borrowed(file_ids))
        } else {
            let file_count = self.index.get_file_count(group_id);
            let file_ids = (0..file_count).collect();
            Some(Cow::Owned(file_ids))
        }
    }

    pub fn is_group_valid(&self, group_id: u32) -> bool {
        group_id < self.index.group_capacity
            && self.index.group_file_capacities[group_id as usize] > 0
    }

    pub fn is_file_valid(&self, group_id: u32, file_id: u32) -> bool {
        group_id < self.index.group_capacity
            && file_id < self.index.group_file_capacities[group_id as usize]
    }

    pub fn is_valid(&self, id: u32) -> bool {
        if self.index.group_count == 1 {
            self.is_file_valid(0, id)
        } else if !self.is_group_valid(id) {
            false
        } else if self.index.get_file_count(id) == 1 {
            self.is_file_valid(id, 0)
        } else {
            panic!("Unable to determine if id is group_id or file_id");
        }
    }

    pub fn fetch_group(&self, group_data: &mut Js5GroupData, group_id: u32) {
        group_data.packed = self.provider.fetch_group(group_id);
    }

    pub fn fetch_all(&self) -> bool {
        let mut success = true;

        for &group_id in self.index.group_ids.iter() {
            let mut group_data = self.groups[group_id as usize].lock().unwrap();
            if group_data.packed.is_none() {
                self.fetch_group(&mut group_data, group_id);
                if group_data.packed.is_none() {
                    success = false;
                }
            }
        }

        success
    }

    pub fn is_group_ready(&self, group_id: u32) -> bool {
        if !self.is_group_valid(group_id) {
            return false;
        }
        let mut group_data = self.groups[group_id as usize].lock().unwrap();
        if group_data.packed.is_none() {
            self.fetch_group(&mut group_data, group_id);
            return group_data.packed.is_some();
        }
        true
    }

    pub fn is_file_ready(&self, group_id: u32, file_id: u32) -> bool {
        if !self.is_group_valid(group_id) {
            return false;
        }
        let mut group_data = self.groups[group_id as usize].lock().unwrap();
        if let Some(unpacked) = &group_data.unpacked {
            unpacked[file_id as usize].is_some()
        } else if group_data.packed.is_none() {
            self.fetch_group(&mut group_data, group_id);
            group_data.packed.is_some()
        } else {
            true
        }
    }

    pub fn is_ready(&self, id: u32) -> bool {
        if self.index.group_count == 1 {
            self.is_file_ready(0, id)
        } else if !self.is_group_valid(id) {
            false
        } else if self.index.get_file_count(id) == 1 {
            self.is_file_ready(id, 0)
        } else {
            panic!("Unable to determine if id is group_id or file_id");
        }
    }

    fn unpack_group(&self, group_data: &mut Js5GroupData, group_id: u32, file_id: u32) -> bool {
        if !self.is_group_valid(group_id) {
            return false;
        }
        if group_data.packed.is_none() {
            return false;
        }

        let file_count = self.index.get_file_count(group_id) as usize;
        let file_ids = self.index.get_file_ids(group_id);

        let unpacked = group_data
            .unpacked
            .get_or_insert_with(|| vec![None; self.index.get_file_capacity(group_id) as usize]);

        let valid = (0..file_count)
            .map(|i| match file_ids {
                Some(ids) => ids[i] as usize,
                None => i,
            })
            .all(|id| unpacked[id].is_some());

        if valid {
            return true;
        }

        let decompressed = {
            let packed = group_data.packed.as_ref().unwrap();
            decompress(packed)
        };

        if self.discard_packed {
            group_data.packed = None;
        }

        if file_count <= 1 {
            let id = match file_ids {
                Some(ids) => ids[0] as usize,
                None => 0,
            };
            unpacked[id] = Some(Bytes::from(decompressed));
        } else {
            let length = decompressed.len();
            let chunks = decompressed[length - 1] as usize;
            let mut file_sizes = vec![0; file_count];
            let mut meta_buf: &[u8] = &decompressed;
            meta_buf.skip(length - 1 - file_count * chunks * 4);

            for _ in 0..chunks {
                let mut file_size = 0;
                for j in 0..file_count {
                    file_size += meta_buf.g4s();
                    file_sizes[j] += file_size;
                }
            }

            meta_buf = &decompressed;
            meta_buf.skip(length - 1 - file_count * chunks * 4);

            let mut files: Vec<Vec<u8>> = file_sizes
                .into_iter()
                .map(|file_size| Vec::with_capacity(file_size as usize))
                .collect();

            let mut data_buf: &[u8] = &decompressed;

            for _ in 0..chunks {
                let mut file_size = 0;
                for j in 0..file_count {
                    file_size += meta_buf.g4s();

                    files[j].extend_from_slice(&data_buf[..file_size as usize]);
                    data_buf.skip(file_size as usize);
                }
            }

            files.into_iter().enumerate().for_each(|(i, file)| {
                let file_id = match file_ids {
                    Some(ids) => ids[i] as usize,
                    None => i,
                };
                unpacked[file_id] = Some(Bytes::from(file));
            });
        }

        true
    }

    pub fn get_file(&self, group_id: u32, file_id: u32) -> Option<Bytes> {
        if !self.is_file_valid(group_id, file_id) {
            return None;
        }

        let mut group_data = self.groups[group_id as usize].lock().unwrap();
        let is_unpacked_file_ready = match group_data.unpacked {
            Some(ref unpacked) => unpacked[file_id as usize].is_some(),
            None => false,
        };
        if !is_unpacked_file_ready {
            if !self.unpack_group(&mut group_data, group_id, file_id) {
                self.fetch_group(&mut group_data, group_id);
                if !self.unpack_group(&mut group_data, group_id, file_id) {
                    return None;
                }
            }
        }

        let unpacked_files = group_data.unpacked.as_mut().unwrap();

        let file = unpacked_files[file_id as usize].as_ref().cloned();

        if file.is_some() && self.discard_unpacked {
            if self.index.get_file_count(group_id) == 1 {
                group_data.unpacked = None;
            } else {
                unpacked_files[file_id as usize] = None;
            }
        }

        file
    }
}
