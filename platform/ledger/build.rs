//! Compile the ledger gRPC contract at build time. `protox` is a pure-Rust
//! protobuf compiler so no system `protoc` install is needed (this repo is
//! built on machines without it).
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/ledger.proto");

    let proto = "proto/ledger.proto";
    let file_descriptors = protox::compile([proto], ["proto/"])?;

    // Hand the validated descriptor set to tonic-build so it can generate
    // code without ever invoking protoc.
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("ledger_descriptor.bin");
    std::fs::write(
        &descriptor_path,
        prost::Message::encode_to_vec(&file_descriptors),
    )?;

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .skip_protoc_run()
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&[proto], &["proto/"])?;
    Ok(())
}
