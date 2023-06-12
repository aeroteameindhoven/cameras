#!/bin/bash

set -eux

curr_date=$(date "+%Y%m%d-%H%M%S")
dir_path="/media/aero/A7C0-F91B/$curr_date"
mkdir $dir_path
libcamera-vid --segment 10 -t 0 --autofocus-mode auto --framerate 60 --level 4.2 --denoise cdn_off --width 1920 --height 1080 -o $dir_path/video.h264 --save-pts $dir_path/timestamps.txt -n
mkvmerge -o $dir_path/final_video.mkv --timecodes 0:$dir_path/timestamps.txt $dir_path/video.h264
