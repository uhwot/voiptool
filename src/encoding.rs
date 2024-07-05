use std::{io::Write, path::Path};

use byteorder::WriteBytesExt;
use crate::resource_parse::ResrcRevision;
use crate::resource_write::write_resource;
use speex_safe::{ControlFunctions, NbMode, NbSubmodeId, SpeexBits, SpeexEncoder};

use crate::{submode_bits_per_frame, SAMPLE_COUNT};

pub fn encode(
    input_samples: Vec<f32>,
    output: &Path,
    quality: i32,
    complexity: i32,
    vad: bool,
    highpass_filter: bool,
    revision: ResrcRevision
) {
    let mut data = Vec::new();

    let mut encoder = SpeexEncoder::<NbMode>::new();
    // qualities over 8 crash the game lol
    encoder.set_quality(quality);
    encoder.set_complexity(complexity);
    // submodes over high also crash the game lol
    //encoder.set_submode(NbSubmodeId::VeryHigh);
    encoder.set_vad(vad);
    encoder.set_highpass(highpass_filter);

    let mut frame_buffer = [0u8; (submode_bits_per_frame(NbSubmodeId::High) as usize + 7) >> 3];
    let mut frame = [0f32; SAMPLE_COUNT];
    
    for chunk in input_samples.chunks(SAMPLE_COUNT) {
        for (i, sample) in chunk.iter().enumerate() {
            frame[i] = sample * 32768.0;
        }

        let mut bits = SpeexBits::new();
        encoder.encode(&mut frame, &mut bits);

        let length = bits.write(&mut frame_buffer);

        let submode = encoder.get_submode();

        let bits_per_frame = submode_bits_per_frame(submode);
        let bytes_per_frame = (bits_per_frame + 7) >> 3;
        if bytes_per_frame != length as u16 {
            panic!("encoded frame length doesn't match submode bytes per frame");
        }

        let mut flags = (submode as i32) as u8;

        if vad {
            if let NbSubmodeId::High = submode {
                flags |= 0x80;
            }

            /*if encoder.get_relative_quality() < 100.0 {
                flags |= 0x40;
            }*/
        } else {
            flags |= 0x80;
        }

        data.write_u8(flags).unwrap();
        data.write_all(&frame_buffer[..length as usize]).unwrap();
    }

    let mut final_data = Vec::new();

    let mut size = data.len();
    loop {
        let mut b = size as u8 & 0x7F;
        size >>= 7;
        if size != 0 {
            b |= 0x80;
        }
        final_data.write_u8(b).unwrap();

        if size == 0 {
            break;
        }
    };

    final_data.write_all(&data).unwrap();

    write_resource(output, final_data, revision);
}