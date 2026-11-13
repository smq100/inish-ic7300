# IC-7300 Audio / Noise Monitor (Rust)

Menu-driven CLI prototype using a service-oriented, object-style architecture (`Application`, `AudioService`, `AudioInput`, `NoiseAnalyzer`). No CAT control is required. The Icom IC-7300 USB audio input appears as an operating-system audio device, often named **USB Audio CODEC**.

## Build and run

Install the Rust toolchain from https://rustup.rs/ and run:

```sh
cargo run --release
```

On Linux, CPAL's ALSA backend may require the system development package `libasound2-dev` (Debian/Ubuntu). On Windows, choose the USB Audio CODEC **input** device, not its output/speaker counterpart.

## Menu

1. List input devices (indexes are enumerated each time; re-check after reconnecting hardware).
2. Connect to a device and start recording into a rolling in-memory buffer.
3. Display a live, one-second-updated noise measurement. Press Enter to return to menu.
4. Disconnect.
5. Configure frequency limits and rolling-window duration.
0. Exit.

## Measurements

- **Band dBFS:** total Welch-estimated audio power within the configured band, relative to full-scale digital amplitude. Includes both signals and noise.
- **Median dBFS/Hz:** median one-sided Welch PSD over the band, a useful *relative* broadband noise proxy; not a calibrated RF noise floor.
- **Peak dBFS:** peak absolute sample amplitude in the rolling window (clipping diagnostic).

Defaults: 300–2700 Hz, five-second rolling window, Hann window, 4096-point FFT, 50% overlap. Stereo/multichannel audio is averaged to mono. Keep IC-7300 audio gain, USB AF output level, AGC, filtering, and Windows input gain identical when comparing choke/no-choke runs. Avoid strong FT8 signals if you want to characterize background noise; alternatively compare median PSD across repeated A/B/A trials.

## Architecture / extension points

- `src/app.rs`: menu, state, orchestration.
- `src/audio.rs`: audio device discovery, connection, stream lifecycle, sample buffering.
- `src/analyzer.rs`: DSP/metrics, independent of hardware and UI, includes unit tests.
- `src/main.rs`: application entry point.

Future extensions: CSV logging, waterfall/spectrum display, WAV recording, device persistence, CAT via serialport/rigwright, multiple analysis strategies, and automatic A/B experiment summaries.

## Notes

- This is an initial prototype. Audio APIs can differ by platform; test against your hardware.
- `cargo test` runs analyzer unit tests.
- Do **not** connect an RF antenna/coax directly to the computer's audio input; use the IC-7300 USB audio interface.
