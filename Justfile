_:
    just --list

# Cross-compile the applocation for raspberry-pi
build:
    cross build

pi_ip := "10.42.0.56"

# Upload over SCP
upload: build
    scp target/armv7-unknown-linux-gnueabihf/debug/cameras aero@{{pi_ip}}:/home/aero/cameras/cameras

set positional-arguments

# Run over SSH
run *args: build upload
    ssh -t aero@{{pi_ip}} "/home/aero/cameras/cameras $@"

ssh:
    ssh aero@{{pi_ip}}