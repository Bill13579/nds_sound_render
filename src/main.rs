use std::{fs::File, sync::Arc};
use rustysynth::{SoundFont, SynthesizerSettings, Synthesizer, MidiFileSequencer, MidiFile};
use hound;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sf2 = File::open("out_bgm.sf2")?;
    let sound_font = Arc::new(SoundFont::new(&mut sf2)?);

    let mut mid = File::open("2_B_SYS_MENU.mid")?;
    let midi_file = Arc::new(MidiFile::new(&mut mid)?);

    let mut settings = SynthesizerSettings::new(32729);
    settings.enable_reverb_and_chorus = false;
    let synthesizer = Synthesizer::new(&sound_font, &settings)?;
    let mut sequencer = MidiFileSequencer::new(synthesizer);

    sequencer.play(&midi_file, false);

    let sample_count = (settings.sample_rate as f64 * midi_file.get_length()) as usize;
    let mut left: Vec<f32> = vec![0_f32; sample_count];
    let mut right: Vec<f32> = vec![0_f32; sample_count];

    sequencer.render(&mut left, &mut right);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 32729,//32768
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create("out.wav", spec)?;
    for (&l, &r) in left.iter().zip(right.iter()) {
        let l = (l * 511.0_f32).round() / 511.0_f32;
        let r = (r * 511.0_f32).round() / 511.0_f32;
        writer.write_sample(l);
        writer.write_sample(r);
    }

    Ok(())
}
