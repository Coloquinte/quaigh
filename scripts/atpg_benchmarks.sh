#!/bin/bash

dirs="atpg"

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
	determinism_output_file="atpg/${name}_check.test"
	quaigh atpg "${benchmark}" -o "${determinism_output_file}" || { echo "ATPG failure on ${name}"; exit 1; }
	diff "${output_file}" "${determinism_output_file}" || { echo "ATPG determinism failure on ${name}"; exit 1; }
done

