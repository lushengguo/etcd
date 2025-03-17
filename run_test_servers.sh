#!/bin/bash

# Ensure log directory exists
mkdir -p logs

# Terminate any potentially running server processes
pkill -f "cargo run --bin server"

# Wait for processes to fully terminate
sleep 1

# 清空日志目录
rm -rf logs/*

# 先编译程序
echo "编译服务器..."
cargo build --bin server

# Start server nodes
echo "Starting node1 (127.0.0.1:2379)..."
RUST_LOG=info ./target/debug/server --node-id 1 --etcd-port 2379 --raft-port 10001 --cluster-conf "1=127.0.0.1:10001,2=127.0.0.1:10002" > logs/node1.log 2>&1 &
NODE1_PID=$!

echo "Starting node2 (127.0.0.1:2380)..."
RUST_LOG=info ./target/debug/server --node-id 2 --etcd-port 2380 --raft-port 10002 --cluster-conf "1=127.0.0.1:10001,2=127.0.0.1:10002" > logs/node2.log 2>&1 &
NODE2_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 5

echo "Server started, PID: $NODE1_PID, $NODE2_PID"
echo "Running tests..."

# 编译并运行客户端
echo "编译客户端..."
cargo build --bin client
./target/debug/client --addr 127.0.0.1:2379

# Terminate servers after testing
echo "Tests completed, terminating servers..."
kill $NODE1_PID $NODE2_PID

echo "Done!" 