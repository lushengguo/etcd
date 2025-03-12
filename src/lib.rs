pub mod raft;

pub mod proto {

    tonic::include_proto!("etcd");
}

pub mod raft_proto {
    tonic::include_proto!("raft");
}

pub mod etcd_rpc;
