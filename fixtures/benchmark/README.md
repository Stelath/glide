# Benchmark Audio Fixtures

These WAV files are developer benchmark fixtures converted from local Voice Memos recordings.
They are mono, 16 kHz, signed 16-bit PCM, matching Glide's recorder output format.

- `dictation-short.wav`: short dictation sample, about 6.9 seconds.
- `dictation-long.wav`: longer dictation sample, about 18.3 seconds.

Use them with `just bench-flow-short`, `just bench-flow-long`, or pass either path to
`glide-bench stt --audio` / `glide-bench flow --audio`.
