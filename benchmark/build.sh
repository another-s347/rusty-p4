#!/bin/bash

mkdir -p build
p4c-bm2-ss --p4v 16 --p4runtime-files build/benchmark.p4.p4info.bin -o build/benchmark.json benchmark.p4
p4c-bm2-ss --p4v 16 --p4runtime-files build/benchmark.p4.p4info.txt -o build/benchmark.json benchmark.p4