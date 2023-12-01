#!/bin/bash

dirs="opt"
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
	echo "Running benchmark ${name} with safe check"
	output_file="opt/${name}.bench"
	quaigh opt "${benchmark}" -o "${output_file}" || { echo "Optimization failure on ${name}"; exit 1; }
	quaigh equiv "${benchmark}" "${output_file}" -c 5 --sat-only || { echo "Equivalence failure on ${name}"; exit 1; }
done

for benchmark in bench/b*.bench bench/s*.bench
do
        name=$(basename "${benchmark}" .bench)
	echo "Running benchmark ${name} with fast check"
	output_file="opt/${name}.bench"
	quaigh opt "${benchmark}" -o "${output_file}" || { echo "Optimization failure on ${name}"; exit 1; }
	quaigh equiv "${benchmark}" "${output_file}" -c 5 || { echo "Equivalence failure on ${name}"; exit 1; }
done
