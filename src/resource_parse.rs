use std::{fs::File, io::{Read, Seek, SeekFrom, Write}};

use byteorder::{BigEndian, ReadBytesExt};
use miniz_oxide::inflate::core::{decompress, inflate_flags::{TINFL_FLAG_PARSE_ZLIB_HEADER, TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF}, DecompressorOxide};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Resrc {
    pub resrc_type: [u8; 3],
    pub method: ResrcMethod,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ResrcRevision {
    pub head: u32,
    pub branch_id: u16,
    pub branch_revision: u16,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ResrcMethod {
    Null,
    Binary {
        resrc_type: [u8; 3],
        revision: ResrcRevision,
        is_encrypted: bool,
        data: Vec<u8>,
    },
}

impl Resrc {
    pub fn new(res: &mut File) -> Self {
        let mut resrc_type = [0u8; 3];
        res.read_exact(&mut resrc_type).unwrap();

        let method = res.read_u8().unwrap();

        let method = match method {
            b'b' | b'e' => {
                let mut rev = ResrcRevision {
                    head: res.read_u32::<BigEndian>().unwrap(),
                    branch_id: 0,
                    branch_revision: 0,
                };
                
                let mut dep_table_offset = None;
                let mut is_compressed = false;
                if rev.head >= 0x109 {
                    dep_table_offset = Some(res.read_u32::<BigEndian>().unwrap());

                    if rev.head >= 0x189 {
                        // normally we should check if the resource type is a static mesh,
                        // but we don't need that crap here lol

                        if rev.head >= 0x271 {
                            rev.branch_id = res.read_u16::<BigEndian>().unwrap();
                            rev.branch_revision = res.read_u16::<BigEndian>().unwrap();
                        }
                        if rev.head >= 0x297 || (rev.head == 0x272 && rev.branch_id == 0x4c44) && rev.branch_revision >= 0x2 {
                            // we can ignore this for voip recordings
                            let _compression_flags = res.read_u8().unwrap();
                        }
                        if res.read_u8().unwrap() != 0 {
                            is_compressed = true;
                        }
                    }
                }

                let data = if is_compressed {
                    zlib_decompress(res)
                } else {
                    let current_pos = res.stream_position().unwrap() as u32;
                    let size = match dep_table_offset {
                        Some(offset) => offset - current_pos,
                        None => res.metadata().unwrap().len() as u32 - current_pos,
                    };
                    let mut data_vec = vec![0u8; size as usize];
                    res.read_exact(&mut data_vec).unwrap();
                    data_vec
                };

                ResrcMethod::Binary {
                    resrc_type,
                    revision: rev,
                    is_encrypted: method == b'e',
                    data,
                }
            },
            _ => { ResrcMethod::Null },
        };

        Self {
            resrc_type,
            method,
        }
    }
}

fn zlib_decompress(res: &mut File) -> Vec<u8> {
    res.seek(SeekFrom::Current(2)).unwrap(); // unused i16, always 0x0001
    let num_chunks = res.read_u16::<BigEndian>().unwrap();

    let mut chunk_infos = Vec::with_capacity(num_chunks as usize);
    let mut total_decompressed_size = 0;

    #[derive(Debug)]
    struct ChunkInfo {
        compressed_size: u16,
        decompressed_size: u16,
    }

    for _ in 0..num_chunks {
        let info = ChunkInfo {
            compressed_size: res.read_u16::<BigEndian>().unwrap(),
            decompressed_size: res.read_u16::<BigEndian>().unwrap(),
        };
        total_decompressed_size += info.decompressed_size as usize;
        chunk_infos.push(info);
    }

    let mut final_data = vec![0u8; total_decompressed_size];

    let mut decompressor = DecompressorOxide::new();

    let mut final_pos = 0;
    for info in chunk_infos {
        let mut deflated_data = vec![0u8; info.compressed_size as usize];
        res.read_exact(&mut deflated_data[..info.compressed_size as usize]).unwrap();

        if info.compressed_size == info.decompressed_size {
            (&mut final_data[final_pos..]).write_all(&deflated_data).unwrap();
        } else {
            let flags = TINFL_FLAG_PARSE_ZLIB_HEADER + TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
            decompress(&mut decompressor, &deflated_data, &mut final_data, final_pos, flags);
            decompressor.init();
        }

        final_pos += info.decompressed_size as usize;
    }

    final_data
}