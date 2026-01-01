#!/bin/bash
cargo ndk -t armeabi-v7a -t arm64-v8a -o $HOME/AndroidStudioProjects/blazeftc/blazeftc/src/main/jniLibs build
POS=${pwd}
cd $HOME/AndroidStudioProjects/blazeftc/blazeftc/src/main/jniLibs
mv ./arm64-v8a/libblaze_ftc_quickstart.so ./arm64-v8a/libblaze_ftc_neutrino.so
mv ./armeabi-v7a/libblaze_ftc_quickstart.so ./armeabi-v7a/libblaze_ftc_neutrino.so
cd $POS
