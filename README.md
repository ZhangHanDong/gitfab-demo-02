# DEMO-02 task API

Requires Rust stable and Cargo. Start with `cargo run --locked --bin demo02`; it listens on `0.0.0.0:8080` by default. Set `PORT` to choose a different port. Data is held in memory and is cleared on restart.

Run `cargo test --locked` for automated tests. Run `python3 tests/acceptance.py` to launch a fresh service and execute the frozen black-box fixture in order. Port 8080 must be free.

Requests larger than 64 KiB return HTTP 413 with `{"error":"payload_too_large"}`.
