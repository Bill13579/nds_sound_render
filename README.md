Given an SF2 soundfont and MIDI's generated through `ppmdu`, `nds_sound_render` renders the MIDI using a modified version of `rustysynth` using no sample interpolation and custom bitcrushing to emulate the audio systems of the Nintendo DS.

## Usage

Build using `cargo`, and afterwards, use `nds_sound_render --help` to see the help menu.

[Examples](./Examples/) to illustrate what the tool does.