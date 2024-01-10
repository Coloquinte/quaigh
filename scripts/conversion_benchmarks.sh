#!/bin/bash

dirs="convert"

cd benchmarks

for dir in $dirs
do
	mkdir -p "${dir}"
done

for benchmark in bench/c*.bench bench/s*.bench bench/b*.bench
do
        name=$(basename "${benchmark}" .bench)
	echo "Running conversion of ${name} from .bench to .blif"
	output_file="convert/${name}.blif"
	quaigh convert "${benchmark}" "${output_file}" || { echo "Optimization failure on ${name}"; exit 1; }
	echo -ne '\tConversion done\n\t'
	quaigh equiv "${benchmark}" "${output_file}" -c 5 || { echo "Equivalence failure on ${name}"; exit 1; }
done

for benchmark in blif/c*.blif blif/s*.blif blif/b*.blif
do
        name=$(basename "${benchmark}" .blif)
	echo "Running conversion of ${name} from .blif to .bench"
	output_file="convert/${name}.bench"
	quaigh convert "${benchmark}" "${output_file}" || { echo "Optimization failure on ${name}"; exit 1; }
	echo -ne '\tConversion done\n\t'
	quaigh equiv "${benchmark}" "${output_file}" -c 5 || { echo "Equivalence failure on ${name}"; exit 1; }
done
