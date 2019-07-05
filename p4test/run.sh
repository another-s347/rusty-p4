#!/usr/bin/env bash

mkdir build
p4c-bm2-ss --p4v 16 --p4runtime-files build/simple.p4.p4info.bin -o build/simple.json simple.p4
sudo python2 run_exercise.py -t topology.json -b simple_switch_grpc