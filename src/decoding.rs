use std::{io::{Cursor, Read}, path::Path};

use byteorder::ReadBytesExt;
use hound::{WavSpec, WavWriter};
use speex_safe::{NbMode, NbSubmodeId, SpeexBits, SpeexDecoder};

use crate::{submode_bits_per_frame, SAMPLE_COUNT};

pub fn decode(data: Vec<u8>, output: &Path) {
    let mut data = Cursor::new(data);

    let mut size: u64 = 0;
    let mut shift = 0;
    loop {
        let b = data.read_u8().unwrap();
        size |= (b as u64 & 0x7F) << shift;

        if (b & 0x80) == 0 {
            break;
        }

        shift += 7;
    }

    size += data.position();
    if size != data.get_ref().len() as u64 {
        panic!("size written in vop data doesn't match actual size, this is probably corrupted");
    }

    let mut decoder = SpeexDecoder::<NbMode>::new();

    let spec = WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = WavWriter::create(output, spec).unwrap();

    let mut frame_buffer = [0u8; (submode_bits_per_frame(NbSubmodeId::High) as usize + 7) >> 3];
    let mut frame = [0f32; SAMPLE_COUNT];
    while data.position() < size {
        let flags = data.read_u8().unwrap();
        //let speech_detected = (flags & 0x80) != 0;
        //let unknown_bit = (flags & 0x40) != 0;
        let op = flags & 0x3F;
        let submode = op.clamp(1, 8);
        let submode = NbSubmodeId::from(submode as i32);

        let bits_per_frame = submode_bits_per_frame(submode);
        let bytes_per_frame = (bits_per_frame + 7) >> 3;

        //println!("submode = {submode:?}, speech = {speech_detected}, unk. bit = {unknown_bit}");

        let buffer = &mut frame_buffer[..bytes_per_frame as usize];
        data.read_exact(buffer).unwrap();
        let mut bits = SpeexBits::new();
        bits.read_from(buffer);

        decoder.decode(&mut bits, &mut frame).unwrap();

        for value in frame {
            writer.write_sample(value / 32768.0).unwrap();
        }
    }

    writer.finalize().unwrap();
}