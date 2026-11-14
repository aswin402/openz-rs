fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/mcp.proto");
    println!("cargo:rerun-if-changed=build.rs");
    tonic_build::compile_protos("proto/mcp.proto")?;
    Ok(())
}
