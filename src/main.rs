use std::{fs::File, path::PathBuf};

use clap::{Parser, Subcommand};
use decoding::decode;
use encoding::encode;
use input_decoding::decode_input;
use resource_parse::{Resrc, ResrcMethod, ResrcRevision};
use speex_safe::NbSubmodeId;

mod encoding;
mod decoding;
mod resource_parse;
mod resource_write;
mod input_decoding;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encodes audio file to VOP
    Encode {
        /// Input file path
        input: PathBuf,
        /// Output file path
        output: PathBuf,
        /// Encoding quality (0 to 8, higher is better)
        #[arg(short, long, default_value_t = 8)]
        quality: i32,
        /// Encoding complexity (0 to 10, higher is better and more CPU intensive)
        #[arg(short, long, default_value_t = 10)]
        complexity: i32,
        /// Enable voice activity detection
        #[arg(short, long, default_value_t = false)]
        vad: bool,
        /// Enable highpass filter
        #[arg(short = 'f', long, default_value_t = false)]
        highpass_filter: bool,
        /// Resource revision
        #[arg(short, long, default_value_t = 0x33e)]
        revision: u32,
        /// Resource branch ID
        #[arg(long, default_value_t = 0x0)]
        branch_id: u16,
        /// Resource branch revision
        #[arg(long, default_value_t = 0x0)]
        branch_revision: u16,
    },
    /// Decodes VOP file to WAV
    Decode {
        /// Input file path
        input: PathBuf,
        /// Output file path
        output: PathBuf,
    },
}


fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Encode {
            input,
            output,
            quality,
            complexity,
            vad,
            highpass_filter,
            revision,
            branch_id,
            branch_revision
        } => {
            if !(0..=8).contains(&quality) {
                println!("Quality has to be between 0 and 8");
                return;
            }

            if !(0..=10).contains(&complexity) {
                println!("Complexity has to be between 0 and 10");
                return;
            }

            let revision = ResrcRevision {
                head: revision,
                branch_id,
                branch_revision,
            };

            let samples = decode_input(&input);
            encode(samples, &output, quality, complexity, vad, highpass_filter, revision)
        },
        Commands::Decode { input, output } => {
            let mut vop = File::open(input).unwrap();
            let vop = Resrc::new(&mut vop);
            if let ResrcMethod::Binary { resrc_type, data, .. } = vop.method {
                assert!(resrc_type == *b"VOP");
                decode(data, &output);
            }
        }
    }
}

const SAMPLE_COUNT: usize = 160;

// speex-safe doesn't support querying for submode bits-per-frame,
// so we will just have to store this crap ourselves ¯\_(ツ)_/¯
// values from here: https://github.com/xiph/speex/blob/1de1260d24e01224df5fbb8b92893106c89bb8de/libspeex/modes.c#L178
const fn submode_bits_per_frame(submode: NbSubmodeId) -> u16 {
    match submode {
        NbSubmodeId::VocoderLike => 43,
        NbSubmodeId::ExtremeLow => 79,
        NbSubmodeId::VeryLow => 119,
        NbSubmodeId::Low => 160,
        NbSubmodeId::Medium => 220,
        NbSubmodeId::High => 300,
        NbSubmodeId::VeryHigh => 364,
        NbSubmodeId::ExtremeHigh => 492,
    }
}
