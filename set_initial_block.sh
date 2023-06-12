#!/bin/bash

initial_block=$(grep -o 'initial_block: [0-9]\+' frens.yaml | awk -F' ' '{print $2}' | sort -n | head -n 1)
sed -i "s/initialBlock: [0-9]\+/initialBlock: $initial_block/g" substreams.yaml
echo "Updated substreams.yaml initialBlock to: $initial_block"
