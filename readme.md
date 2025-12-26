# Blaze FTC Quickstart
The repository for BlazeFTC can be found [here](https://github.com/owenpmckenna/blaze_ftc). It has all the relevant information on what exactly this is, so read its readme before you start here.

To get started, you will need to:
1. add this code block to the `android {}` section of your :TeamCode build.gradle.
```
sourceSets {
   main {
      jniLibs.srcDirs = ["src/main/jniLibs"]
   }
}
```
2. create the folder: `%projectroot%/TeamCode/src/main/jniLibs`. Studio makes this kinda difficult, so I'd just open up the terminal and run `mkdir TeamCode/src/main/jniLibs` from your project's root directory.
3. clone this repository in your environment of choice! If you're considering using this repo, I think you can probably figure this one out.
4. set the build script to the correct target. go to build.sh (no I cannot test this on windows, the command should be the same) and replace the target output directory with yours. (this is easiest to get by right-clicking the jniLibs folder and selecting Copy Absolute Path or Reference).
5. Download cargo ndk! Run `cargo install cargo-ndk` and `rustup target add aarch64-linux-android armv7-linux-androideabi` if you haven't done this before.
6. Read the examples! Seriously. A high level overview is in the main repo but the Robot framework is kind of wierd and reading these will be very helpful.
7. Copy the JVM code into your project. It can be found in the Java folder of this project. Eventually this will be set up correctly, but I don't have a maven build server yet so this is how we're doing it. One of the files is in kotlin, sorry. Also, if you don't have Panels, remove the import and annotations from the code. Note: do not change the package of the BlazeFTC.java file.
8. Run ./build.sh (or just the command in your terminal if you're on windows)
9. Run Teamcode/installRelease in Android Studio
10. Run the BlazeFTC opmode.

If anything breaks, ping me on discord and I will try my best to help.

If you'd like to help with the development of the main project, clone it too and change this repo's Cargo.toml to find blaze_ftc from the local path where you cloned it to.

Examples:
1. the "run_bare" function in lib.rs shows how to make a very simple mecanum opmode, for testing specific features, or if you decide my Robot framework is terrible (fair enough).
2. the `basic_mecanum.rs` opmode shows how to make a basic mecanum opmode, using the gamepad handler function.
3. the `mecanum_with_brakes.rs` opmode is a little trivial in that you would never actually do this. It is normal mecanum, until you press "a" at which time several pid loops activate, with the goal of keeping the robot in place. It shows how to move state between handlers within the framework. Or you can use static. Either way.
4. the `auto_pids.rs` opmode shows how this is actually supposed to be used, with the main thread sending targets (yes it's just pids for now, I do not have a pathing library ready) and waiting for them to be completed. It's implemented a little jankily but you can get the idea of how it is supposed to work.

