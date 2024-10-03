fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/etcd_operation.proto")?;
    tonic_build::compile_protos("proto/raft.proto")?;
    Ok(())
}
