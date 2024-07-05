use std::{fs::File, io::{Seek, Write}, path::Path};

use byteorder::{BigEndian, WriteBytesExt};
use miniz_oxide::deflate::compress_to_vec_zlib;

use crate::resource_parse::ResrcRevision;

pub fn write_resource(path: &Path, data: Vec<u8>, rev: ResrcRevision) {
    let mut out = File::create(path).unwrap();

    // resource header crap

    out.write_all(b"VOPb").unwrap();
    out.write_u32::<BigEndian>(rev.head).unwrap();
    
    if rev.head >= 0x109 {
        // dependency table offset, to be written later
        out.write_u32::<BigEndian>(0).unwrap();

        if rev.head >= 0x189 {
            if rev.head >= 0x271 {
                out.write_u16::<BigEndian>(rev.branch_id).unwrap();
                out.write_u16::<BigEndian>(rev.branch_revision).unwrap();
            }

            if rev.head >= 0x297 || (rev.head == 0x272 && rev.branch_id == 0x4c44) && rev.branch_revision >= 0x2 {
                // compression flags, doesn't matter for voip recordings
                out.write_u8(0x7).unwrap();
            }

            // is zlib compressed
            out.write_u8(1).unwrap();
        }
    }

    // zlib compression

    const CHUNK_SIZE: usize = 0x8000;

    out.write_u16::<BigEndian>(1).unwrap();

    let mut num_chunks = data.len() / CHUNK_SIZE;
    if (data.len() % CHUNK_SIZE) != 0 {
        num_chunks += 1;
    }
    out.write_u16::<BigEndian>(num_chunks as u16).unwrap();

    let mut zlib_chunks = Vec::with_capacity(num_chunks);

    for chunk in data.chunks(CHUNK_SIZE) {
        let compressed = compress_to_vec_zlib(chunk, 9);
        if compressed.len() < chunk.len() {
            out.write_u16::<BigEndian>(compressed.len() as u16).unwrap();
            out.write_u16::<BigEndian>(chunk.len() as u16).unwrap();
            zlib_chunks.push(compressed);
        } else {
            out.write_u16::<BigEndian>(chunk.len() as u16).unwrap();
            out.write_u16::<BigEndian>(chunk.len() as u16).unwrap();
            zlib_chunks.push(chunk.to_vec());
        }
    }

    for chunk in zlib_chunks {
        out.write_all(&chunk).unwrap();
    }

    // dependency table

    if rev.head >= 0x109 {
        let dep_table_offset = out.stream_position().unwrap();

        // num of dependencies
        out.write_u32::<BigEndian>(0).unwrap();

        out.seek(std::io::SeekFrom::Start(8)).unwrap();
        out.write_u32::<BigEndian>(dep_table_offset as u32).unwrap();
    }
}