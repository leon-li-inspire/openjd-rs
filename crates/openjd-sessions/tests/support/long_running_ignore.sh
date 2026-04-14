#!/bin/sh
trap 'echo Trapped' TERM
for i in $(seq 0 19); do
    echo "$i"
    sleep 1
done
