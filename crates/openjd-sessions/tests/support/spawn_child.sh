#!/bin/sh
DIR=$(dirname "$0")
"$DIR/long_running.sh" &
CHILD=$!
wait $CHILD
for i in $(seq 0 19); do
    echo "Log from runner $i"
    sleep 1
done
