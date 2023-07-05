use std::{fs::File, sync::Arc, path::{self, Path}, ffi::OsStr, error::Error, process::ExitCode, f32::consts::E};
use rustysynth::{SoundFont, SynthesizerSettings, Synthesizer, MidiFileSequencer, MidiFile};
use hound;
use std::path::PathBuf;
use clap::{Parser};
use glob::glob;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Sets the path to the `.sf2` Soundfont file
    #[arg(value_name = "SF2")]
    sf2: PathBuf,

    /// Sets the path of the MIDI-file to be rendered
    #[arg(value_name = "INPUT")]
    input_glob: String,

    /// Sets the folder to output rendered wave-files in
    #[arg(short = 'o', long, value_name = "OUTPUT")]
    output_folder: Option<PathBuf>,

    /// Target bit-depth for bit reduction (set to 0 to disable)
    /// 
    /// NDS supports 16-bit audio, but in reality it seems that the internal processing could end up reducing the output bit-depth to 10-bits.
    /// Source: https://www.reddit.com/r/emulation/comments/ru5nld/i_really_love_the_sound_of_the_nintendo_ds/
    #[arg(short = 'b', long, default_value_t = 10)]
    bitdepth: u8,

    /// Target sample rate for zero-interpolation resampling
    /// 
    /// The Nintendo DS's audio systems do not do any interpolation on resampling of audio samples, which means sound coming out of the NDS tend to contain a lot more high-frequency content, a sort of a ringing effect that is awesome, and so to recreate it the audio can be resampled the same way here inside the patched `rustysynth` SF2 player.
    /// Sources indicate different sample rates, but here the one suggested by Wenting Zhang, 32728.5 Hz, is used. https://www.zephray.me/post/nds_3ds_sound_quality/
    /// There is also 32768 Hz, suggested by Justme from https://retrocomputing.stackexchange.com/questions/24952/is-sound-generation-on-the-nintendo-ds-always-clipped-to-10-bits
    #[arg(short = 's', long, default_value_t = 32729)]
    sample_rate: u32,

    /// How many times to repeat the midi files
    #[arg(short = 'r', long, default_value_t = 1.0)]
    repeat: f64
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut sf2 = File::open(cli.sf2)?;
    let sound_font = Arc::new(SoundFont::new(&mut sf2)?);

    let output_folder;
    if let Some(custom_output_folder) = cli.output_folder {
        if std::fs::metadata(&custom_output_folder)?.is_dir() {
            output_folder = custom_output_folder;
        } else {
            return Err("Output path must be a folder!".into());
        }
    } else {
        output_folder = std::env::current_dir()?;
    }

    fn valid_midi_file<P: AsRef<Path>>(path: P) -> bool {
            if let Ok(file_metadata) = std::fs::metadata(&path) {
                let is_file = file_metadata.is_file();
                let extension = path.as_ref().extension();
                if let Some(extension) = extension {
                    if let Some(extension) = extension.to_str() {
                        is_file && extension == "mid"
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
    }
    let input_file_paths: Vec<(PathBuf, PathBuf)> = glob(&cli.input_glob).expect("Failed to read glob pattern").into_iter().filter_map(|entry| {
        match entry {
            Ok(path) => {
                if !valid_midi_file(&path) {
                    println!("Skipping {}!", path.display());
                    None
                } else {
                    if let Some(input_file_name) = path.file_name() {
                        let mut output_path = output_folder.clone();
                        PathBuf::push(&mut output_path, input_file_name);
                        output_path.set_extension("wav");
                        Some((path, output_path))
                    } else {
                        None
                    }
                }
            },
            Err(e) => {
                println!("{:?}", e);
                None
            }
        }
    }).collect();

    // sound_font - Loaded Soundfont
    // input_file_paths - MIDI files to render and where to render them to
    // output_folder - Output path
    // bitdepth - Target bit-depth for bit reduction
    // sample_rate - Target sample rate for zero-interpolation resampling

    for (input_file_path, output_file_path) in input_file_paths {
        print!("Rendering {}... ", input_file_path.display());
        render(sound_font.clone(), input_file_path, output_file_path, cli.bitdepth, cli.sample_rate, cli.repeat)?;
        println!("done!");
    }

    println!("\nFriendly Friends!~ Keep up your training!\n\n");

    Ok(())
}

pub fn render<P: AsRef<Path>>(sound_font: Arc<SoundFont>, input_file_path: P, output_file_path: P, bitdepth: u8, sample_rate: u32, repeat: f64) -> Result<(), Box<dyn std::error::Error>> {
    let mut mid = File::open(input_file_path)?;
    let midi_file = Arc::new(MidiFile::new(&mut mid)?);

    let mut settings = SynthesizerSettings::new(sample_rate as i32);
    settings.enable_reverb_and_chorus = false;
    let synthesizer = Synthesizer::new(&sound_font, &settings)?;
    let mut sequencer = MidiFileSequencer::new(synthesizer);

    sequencer.play(&midi_file, if repeat == 1.0 { false } else { true });

    let sample_count = (settings.sample_rate as f64 * midi_file.get_length() * repeat) as usize;
    let mut left: Vec<f32> = vec![0_f32; sample_count];
    let mut right: Vec<f32> = vec![0_f32; sample_count];

    sequencer.render(&mut left, &mut right);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output_file_path, spec)?;
    for (&(mut l), &(mut r)) in left.iter().zip(right.iter()) {
        if bitdepth != 0 {
            l = quantize_to_bitdepth(l, bitdepth);
            r = quantize_to_bitdepth(r, bitdepth);
        }
        writer.write_sample(l)?;
        writer.write_sample(r)?;
    }

    Ok(())
}

pub fn quantize_to_bitdepth(x: f32, bitdepth: u8) -> f32 {
    quantize_f32(x, 2_u32.pow(bitdepth as u32 - 1) - 1)
}

/// A simple linear quantization of a floating-point number `x` within a range of [-1.0, 1.0] by projecting the number onto a range of integers [-`n_half`, `n_half`]
/// 
/// Note
/// ====
/// For quantizing a 32-bit floating point number to an `n`-bit floating point number, set `n_half` to be 
/// `n_half = 2^(n-1) - 1`
pub fn quantize_f32(x: f32, n_half: u32) -> f32 {
    (x * n_half as f32).round() / n_half as f32
}
