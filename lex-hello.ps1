[System.Environment]::SetEnvironmentVariable("ORCHID_LOG_BUFFERS", "true")
cargo build -p orchid-std
cargo run -p orcx -- `
    --extension .\target\debug\orchid-std.exe --system orchid::std `
    lex --file .\examples\hello-world\main.orc
