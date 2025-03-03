#!/bin/bash

# 确保日志目录存在
mkdir -p log

# 终止可能已经运行的服务器进程
pkill -f "server.*2379" || true
pkill -f "server.*2380" || true

# 等待进程完全终止
sleep 1

# 启动两个服务器节点
echo "启动节点1 (127.0.0.1:2379)..."
cargo run --bin server -- 127.0.0.1:2379 raft_configuration.json 1 > log/node1.log 2>&1 &
NODE1_PID=$!

echo "启动节点2 (127.0.0.1:2380)..."
cargo run --bin server -- 127.0.0.1:2380 raft_configuration.json 2 > log/node2.log 2>&1 &
NODE2_PID=$!

# 等待服务器启动
echo "等待服务器启动..."
sleep 3

echo "服务器已启动，PID: $NODE1_PID, $NODE2_PID"
echo "运行测试..."

# 运行测试
cargo test -- --nocapture

# 测试完成后终止服务器
echo "测试完成，终止服务器..."
kill $NODE1_PID $NODE2_PID

echo "完成！" 