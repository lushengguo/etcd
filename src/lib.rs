pub mod raft;

// 包含生成的 protobuf 代码
pub mod proto {
    // 这里直接引入生成的代码，不需要使用 include! 宏
    // 编译时 tonic-build 会自动生成并包含
    tonic::include_proto!("etcd");
}

// 保留 etcd_rpc 模块（但将其实现修改为基于 tonic）
pub mod etcd_rpc;