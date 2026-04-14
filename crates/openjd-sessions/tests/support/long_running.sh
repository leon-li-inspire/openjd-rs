#!/bin/sh
trap 'echo Trapped; exit 1' TERM
for i in $(seq 0 19); do
    echo "Log from test $i"
    sleep 1
done
