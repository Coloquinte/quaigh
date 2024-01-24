#!/bin/bash

dirs="opt"

cd benchmarks

for dir in $dirs
do
	mkdir -p "${dir}"
done

for benchmark in bench/iscas*.bench
do
        name=$(basename "${benchmark}" .bench)
	echo "Running benchmark ${name}"
	output_file="opt/${name}.bench"
	quaigh opt "${benchmark}" -o "${output_file}" || { echo "Optimization failure on ${name}"; exit 1; }
	determinism_output_file="opt/${name}_check.bench"
	quaigh opt "${benchmark}" -o "${determinism_output_file}" || { echo "Optimization failure on ${name}"; exit 1; }
	diff "${output_file}" "${determinism_output_file}" || { echo "Optimization determinism failure on ${name}"; exit 1; }
	echo "Initial stats:"
	quaigh show "${benchmark}"
	echo "Final stats:"
	quaigh show "${output_file}"
	echo -ne '\tOptimization done\n\t'
	quaigh equiv "${benchmark}" "${output_file}" -c 5 || { echo "Equivalence failure on ${name}"; exit 1; }
done
