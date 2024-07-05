use std::{fs::File, path::Path};

use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};

use symphonia::core::{audio::SampleBuffer, codecs::CODEC_TYPE_NULL};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const SPEEX_SAMPLE_RATE: u32 = 8000;

// code mostly based on https://github.com/pdeljanov/Symphonia/blob/master/symphonia/examples/basic-interleaved.rs
pub fn decode_input(path: &Path) -> Vec<f32> {
    let file = Box::new(File::open(path).unwrap());

    let mss = MediaSourceStream::new(file, Default::default());

    let hint = Hint::new();

    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .expect("unsupported format");

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    let sample_rate = track.codec_params.sample_rate.unwrap();
    let num_channels = match track.codec_params.channels {
        Some(channels) => channels.count(),
        None => 1,
    };

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .expect("unsupported codec");

    // Store the track identifier, we'll use it to filter packets.
    let track_id = track.id;

    let mut sample_buf = None;
    let mut final_samples = Vec::new();

    loop {
        // Get the next packet from the format reader.
        let packet = format.next_packet();
        let packet = match packet {
            Ok(p) => p,
            Err(_) => break
        };

        // If the packet does not belong to the selected track, skip it.
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                if sample_buf.is_none() {
                    let spec = *audio_buf.spec();

                    let duration = audio_buf.capacity() as u64;

                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);

                    // multiple channel to mono conversion
                    for samples in buf.samples().chunks(num_channels) {
                        let mut average = 0.0;
                        for sample in samples {
                            average += sample;
                        }
                        average /= num_channels as f32;
                        final_samples.push(average);
                    }
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    match sample_rate == SPEEX_SAMPLE_RATE {
        true => final_samples,
        false => resample(final_samples, sample_rate)
    }
}

fn resample(indata: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    // time for some stupid ass resampling code which i barely even understand :)))
    // based on https://github.com/HEnquist/rubato/blob/master/examples/process_f64.rs

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let rate_in = sample_rate;

    let mut resampler = SincFixedIn::<f32>::new(
        SPEEX_SAMPLE_RATE as f64 / rate_in as f64,
        2.0,
        params,
        1024,
        1,
    ).unwrap();

    let mut indata = &indata[..];
    let nbr_input_frames = indata.len();

    let mut outdata = Vec::with_capacity(
        2 * (nbr_input_frames as f32 * SPEEX_SAMPLE_RATE as f32 / rate_in as f32) as usize
    );

    let mut input_frames_next = resampler.input_frames_next();
    let resampler_delay = resampler.output_delay();
    let mut outbuffer = [vec![0.0f32; resampler.output_frames_max()]];

    while indata.len() >= input_frames_next {
        let (nbr_in, nbr_out) = resampler
            .process_into_buffer(&[&indata], &mut outbuffer, None)
            .unwrap();
        indata = &indata[nbr_in..];
        outdata.extend(&outbuffer[0][..nbr_out]);
        input_frames_next = resampler.input_frames_next();
    }

    // Process a partial chunk with the last frames.
    if !indata.is_empty() {
        let (_nbr_in, nbr_out) = resampler
            .process_partial_into_buffer(Some(&[&indata]), &mut outbuffer, None)
            .unwrap();

        outdata.extend(&outbuffer[0][..nbr_out]);
    }

    let nbr_output_frames = (nbr_input_frames as f32 * SPEEX_SAMPLE_RATE as f32 / rate_in as f32) as usize;
    let end = (resampler_delay + nbr_output_frames).min(outdata.len() - 1);

    let outdata = &outdata[resampler_delay..end];
    outdata.to_vec()
}