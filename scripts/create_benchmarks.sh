#!/bin/bash
# Original script to get and extract the benchmarks

mkdir -p benchmarks/bench
mkdir -p benchmarks/blif

cd benchmarks

cd bench
for bench in iscas85 iscas89 iscas99
do
	wget https://pld.ttu.ee/~maksim/benchmarks/${bench}/bench -qN -l1 -nH -np -r --cut-dirs=4 --reject="*.html*" --reject=robots.txt
	rm bench
done
cd ..

cd blif
for bench in $(ls ../bench)
do
	yosys-abc -c "read_bench ../bench/${bench}; write_blif ${bench%%.bench}.blif" > /dev/null
	sed -i "s/new_//g" "${bench%%.bench}.blif"
done
rm abc.history
cd ..

cd ..
