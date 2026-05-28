# Glide - desktop build system
# Install: brew install just  (or: cargo install just)
# Usage:   just              (list all recipes)
#          just dev           (debug build + run for current OS)
#          just build         (release build + package for current OS)

ROOT := justfile_directory()
current_os := if os() == "macos" { "mac" } else { os() }

# List available recipes
default:
    @just --list

# ─── Primary Commands ─────────────────────────────────────────────────────────

# Debug build + auto-run (platform: mac, linux, windows)
dev platform=current_os:
    @just _dev-{{platform}}

# Release build + package artifacts (platform: mac, linux, windows)
build platform=current_os:
    @just _build-{{platform}}

# ─── macOS ────────────────────────────────────────────────────────────────────

[private]
_dev-mac:
    cargo run

[private]
_build-mac:
    {{ROOT}}/bundle.sh

# Debug app bundle, requiring a valid Apple code-signing identity.
dev-signed: _stop-dev-signed
    rm -f "{{ROOT}}/target/debug/Glide" "{{ROOT}}/target/debug/Glide.app/Contents/MacOS/Glide"
    {{ROOT}}/bundle.sh --debug --sign
    open {{ROOT}}/target/debug/Glide.app

[private]
_stop-dev-signed:
    # [G] keeps pgrep/pkill from matching this recipe's shell command.
    -pkill -TERM -f "{{ROOT}}/target/debug/[G]lide.app/Contents/MacOS/Glide"
    @for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do \
        pgrep -f "{{ROOT}}/target/debug/[G]lide.app/Contents/MacOS/Glide" >/dev/null || exit 0; \
        sleep 0.1; \
    done; \
    pkill -KILL -f "{{ROOT}}/target/debug/[G]lide.app/Contents/MacOS/Glide" || true

# ─── Linux ────────────────────────────────────────────────────────────────────

[private]
_dev-linux:
    cargo run

[private]
_build-linux:
    cargo build --release

# ─── Windows ──────────────────────────────────────────────────────────────────

[private]
_dev-windows:
    cargo run

[private]
_build-windows:
    cargo build --release

# ─── Setup ────────────────────────────────────────────────────────────────────

# Check and install required tools
setup:
    {{ROOT}}/setup.sh

# ─── Quality ──────────────────────────────────────────────────────────────────

# Run all tests
test:
    cargo test

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format all code
fmt:
    cargo fmt --all

# Check compilation without building
check:
    cargo check

# Benchmark a speech-to-text provider against a WAV file.
bench-stt audio provider="openai" model="whisper-1" runs="3" warmups="1":
    cargo run --bin glide-bench -- stt --audio "{{audio}}" --provider "{{provider}}" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Benchmark an LLM cleanup provider against text or @file input.
bench-llm text provider="openai" model="gpt-5.4-nano" runs="3" warmups="1":
    cargo run --bin glide-bench -- llm --text "{{text}}" --provider "{{provider}}" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Benchmark Glide's dictation flow without pasting by default.
bench-flow audio runs="3" warmups="1":
    cargo run --bin glide-bench -- flow --audio "{{audio}}" --runs "{{runs}}" --warmups "{{warmups}}" --no-paste

# Benchmark Glide's dictation flow with the short recorded fixture.
bench-flow-short runs="3" warmups="1":
    cargo run --bin glide-bench -- flow --audio "{{ROOT}}/fixtures/benchmark/dictation-short.wav" --runs "{{runs}}" --warmups "{{warmups}}" --no-paste

# Benchmark Glide's dictation flow with the long recorded fixture.
bench-flow-long runs="3" warmups="1":
    cargo run --bin glide-bench -- flow --audio "{{ROOT}}/fixtures/benchmark/dictation-long.wav" --runs "{{runs}}" --warmups "{{warmups}}" --no-paste

# Build and run the signed debug app in the foreground with real dictation JSONL tracing enabled.
profile-app-trace output="target/glide-bench/app-trace.jsonl":
    mkdir -p "{{ROOT}}/target/glide-bench"
    {{ROOT}}/bundle.sh --debug --sign
    GLIDE_TRACE=1 GLIDE_TRACE_PATH="{{output}}" "{{ROOT}}/target/debug/Glide.app/Contents/MacOS/Glide"

# Run the signed debug app under Apple's Time Profiler and also emit Glide JSONL trace spans.
profile-app-xctrace duration="30s" output_dir="target/glide-bench":
    mkdir -p "{{output_dir}}"
    {{ROOT}}/bundle.sh --debug --sign
    xcrun xctrace record --template "Time Profiler" --time-limit "{{duration}}" --output "{{output_dir}}" --env GLIDE_TRACE=1 --env GLIDE_TRACE_PATH="{{output_dir}}/app-xctrace.jsonl" --launch -- "{{ROOT}}/target/debug/Glide.app/Contents/MacOS/Glide"

# Benchmark an STT provider with the short recorded fixture.
bench-stt-short provider="parakeet" model="parakeet-tdt-0.6b-v3-int8" runs="3" warmups="1":
    cargo run --bin glide-bench -- stt --audio "{{ROOT}}/fixtures/benchmark/dictation-short.wav" --provider "{{provider}}" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Benchmark Fireworks Whisper Turbo with the short recorded fixture.
bench-stt-fireworks model="whisper-v3-turbo" runs="3" warmups="1":
    cargo run --bin glide-bench -- stt --audio "{{ROOT}}/fixtures/benchmark/dictation-short.wav" --provider "fireworks" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Benchmark ElevenLabs Scribe with the short recorded fixture.
bench-stt-elevenlabs model="scribe_v2" runs="3" warmups="1":
    cargo run --bin glide-bench -- stt --audio "{{ROOT}}/fixtures/benchmark/dictation-short.wav" --provider "elevenlabs" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Benchmark a Fireworks serverless/API LLM against text or @file input.
bench-llm-fireworks text model="accounts/fireworks/models/gpt-oss-20b" runs="3" warmups="1":
    cargo run --bin glide-bench -- llm --text "{{text}}" --provider "fireworks" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Benchmark an STT provider with the long recorded fixture.
bench-stt-long provider="parakeet" model="parakeet-tdt-0.6b-v3-int8" runs="3" warmups="1":
    cargo run --bin glide-bench -- stt --audio "{{ROOT}}/fixtures/benchmark/dictation-long.wav" --provider "{{provider}}" --model "{{model}}" --runs "{{runs}}" --warmups "{{warmups}}"

# Compare two benchmark JSON reports and fail above a percent threshold.
bench-compare baseline candidate threshold="20":
    cargo run --bin glide-bench -- compare --baseline "{{baseline}}" --candidate "{{candidate}}" --fail-threshold "{{threshold}}"

# Run the live prompt eval suite against the default remote LLM candidates.
prompt-eval-core runs="1" timeout_secs="60":
    cargo run --bin glide-bench -- prompt-eval --suite "{{ROOT}}/fixtures/prompt_eval/core.jsonl" --candidate "openai:gpt-5.4-nano" --candidate "groq:meta-llama/llama-4-scout-17b-16e-instruct" --candidate "cerebras:gpt-oss-120b" --candidate "fireworks:accounts/fireworks/models/gpt-oss-20b" --runs "{{runs}}" --timeout-secs "{{timeout_secs}}"

# Run prompt eval with structured transcript framing but without deterministic edit pre-processing.
prompt-eval-core-structured runs="1" timeout_secs="60":
    cargo run --bin glide-bench -- prompt-eval --suite "{{ROOT}}/fixtures/prompt_eval/core.jsonl" --candidate "openai:gpt-5.4-nano" --candidate "groq:meta-llama/llama-4-scout-17b-16e-instruct" --candidate "cerebras:gpt-oss-120b" --candidate "fireworks:accounts/fireworks/models/gpt-oss-20b" --runs "{{runs}}" --timeout-secs "{{timeout_secs}}" --no-edit-prepass

# ─── Cleanup ──────────────────────────────────────────────────────────────────

# Remove all build artifacts
clean:
    cargo clean
