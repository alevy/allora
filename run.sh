#!/bin/sh

sudo ip tuntap add mode tap tap0
sudo ip addr add dev tap0 192.168.14.1/24
sudo ip link set tap0 up

sudo qemu-system-aarch64 -M virt -cpu cortex-a53 -smp cpus=2 -m 1024M -display none -serial stdio -global virtio-mmio.force-legacy=false -device virtio-rng-device -drive if=none,cache=directsync,file=test.img,format=raw,id=hd0 -device virtio-blk-device,drive=hd0 -netdev type=tap,vhost=on,ifname=tap0,id=net0,script=no,downscript=no -device virtio-net-device,netdev=net0 -kernel $1

sudo ip link delete tap0
