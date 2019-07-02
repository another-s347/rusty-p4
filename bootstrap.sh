#!/usr/bin/env bash

P4RuntimeCommit=e7a10bb

git clone https://github.com/p4lang/p4runtime.git
cd p4runtime && git checkout $P4RuntimeCommit && git apply ../CI.patch
cd ..
mkdir -p src/proto/rust_grpc_out
mkdir -p src/proto/rust_out
./p4runtime/CI/compile_protos.sh ./src/proto
