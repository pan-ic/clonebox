#!/bin/sh
#rsync -av --exclude target . rpi-16-0:/home/pan/clonebox/ && \
#ssh rpi-16-0 "doas lbu commit && source ~/.cargo/env && cd /home/pan/clonebox && cargo test"
rsync -av --exclude target . scaleway:/home/debian/clonebox/ && \
ssh scaleway "source ~/.cargo/env && cd /home/debian/clonebox && cargo test"