#!/bin/bash

dirs="atpg"
time_limit=600
iter_limit=1000000

cd benchmarks

for dir in $dirs
do
	mkdir -p "${dir}"
done

for benchmark in bench/c*.bench
do
        name=$(basename "${benchmark}" .bench)
	echo "Running atpg on ${name}"
	output_file="atpg/${name}.test"
	quaigh atpg "${benchmark}" -o "${output_file}" || { echo "ATPG failure on ${name}"; exit 1; }
done

