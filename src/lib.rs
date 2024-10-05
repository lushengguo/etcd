pub mod raft;
pub mod etcd_protobufs {
    tonic::include_proto!("etcd_protobufs");
}

pub mod raft_protobufs {
    tonic::include_proto!("raft_protobufs");
}