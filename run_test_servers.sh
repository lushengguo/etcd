#!/bin/bash

# Ensure log directory exists
mkdir -p logs

# Terminate any potentially running server processes
pkill -f "cargo run --bin server"

# Wait for processes to fully terminate
sleep 1

# Start two server nodes
echo "Starting node1 (127.0.0.1:2379)..."
RUST_LOG=info cargo run --bin server -- --node-id 1 --etcd-port 2379 --raft-port 10001 --cluster-conf "1=127.0.0.1:10001,2=127.0.0.1:10002" > logs/node1.log 2>&1 &
NODE1_PID=$!

echo "Starting node2 (127.0.0.1:2380)..."
RUST_LOG=info cargo run --bin server -- --node-id 2 --etcd-port 2380 --raft-port 10002 --cluster-conf "1=127.0.0.1:10001,2=127.0.0.1:10002" > logs/node2.log 2>&1 &
NODE2_PID=$!

# Wait for servers to start
echo "Waiting for servers to start..."
sleep 3

echo "Servers started, PID: $NODE1_PID, $NODE2_PID"
echo "Running tests..."

# Run tests
cargo run --bin client -- --addr 127.0.0.1:2379

# Terminate servers after testing
echo "Tests completed, terminating servers..."
kill $NODE1_PID $NODE2_PID

echo "Done!" 