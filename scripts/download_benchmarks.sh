#!/bin/bash
# Obtain a compressed version of the benchmarks

wget -qN https://github.com/Coloquinte/LogicBenchmarks/releases/download/v1.0.1/logic_benchmarks.zip
mkdir benchmarks
unzip logic_benchmarks.zip -d benchmarks
