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
	echo "Checking equivalence between .bench to .blif for benchmark ${name}"
	blif_file="blif/${name}.blif"
	echo -ne '\t'
	quaigh equiv "${benchmark}" "${blif_file}" -c 5 || { echo "Equivalence failure on ${name}"; exit 1; }
done
