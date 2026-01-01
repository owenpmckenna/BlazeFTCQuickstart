# Blaze FTC Quickstart
The repository for BlazeFTC can be found [here](https://github.com/owenpmckenna/blaze_ftc). It has all the relevant information on what exactly this is, so read its readme before you start here.

To get started, you will need to:
1. add `implementation("dev.anygeneric:blazeftc:0.1.2")` to your gradle dependencies

(steps 2-9 may be skipped if all you care about if the Neutrino Proxy)
2. add this code block to the `android {}` section of your :TeamCode build.gradle.
```
sourceSets {
   main {
      jniLibs.srcDirs = ["src/main/jniLibs"]
   }
}
```
3. create the folder: `%projectroot%/TeamCode/src/main/jniLibs`. Studio makes this kinda difficult, so I'd just open up the terminal and run `mkdir TeamCode/src/main/jniLibs` from your project's root directory.
4. clone this repository in your environment of choice! If you're considering using this repo, I think you can probably figure this one out.
5. set the build script to the correct target. go to build.sh (no I cannot test this on windows, the command should be the same) and replace the target output directory with yours. (this is easiest to get by right-clicking the jniLibs folder and selecting Copy Absolute Path or Reference).
6. Download cargo ndk! Run `cargo install cargo-ndk` and `rustup target add aarch64-linux-android armv7-linux-androideabi` if you haven't done this before.
7. Read the examples! Seriously. A high level overview is in the main repo but the Robot framework is kind of wierd and reading these will be very helpful.
9. Run ./build.sh (or just the command in your terminal if you're on windows)
10. create an OpMode or copy the ones at the bottom of the readme
11. Run Teamcode or installRelease in Android Studio
12. Run your OpMode.

If anything breaks, ping me on discord and I will try my best to help.

If you'd like to help with the development of the main project, clone it too and change this repo's Cargo.toml to find blaze_ftc from the local path where you cloned it to.

Examples:
1. the "run_bare" function in lib.rs shows how to make a very simple mecanum opmode, for testing specific features, or if you decide my Robot framework is terrible (fair enough).
2. the `basic_mecanum.rs` opmode shows how to make a basic mecanum opmode, using the gamepad handler function.
3. the `mecanum_with_brakes.rs` opmode is a little trivial in that you would never actually do this. It is normal mecanum, until you press "a" at which time several pid loops activate, with the goal of keeping the robot in place. It shows how to move state between handlers within the framework. Or you can use static. Either way.
4. the `auto_pids.rs` opmode shows how this is actually supposed to be used, with the main thread sending targets (yes it's just pids for now, I do not have a pathing library ready) and waiting for them to be completed. It's implemented a little jankily but you can get the idea of how it is supposed to work.
5. the `neutrino_proxy.rs` opmode shows how to intercept packets from the SDK to the underlying hardware. In its default configuration, it will respond at once with a simple ACK to all motor commands, while also passing them to hardware. This has a similar effect to that of Photon, and it can decrease loop times significantly. Replacing the `handle_packet` code with `println!("packet: {:?}", packet); Some(packet)` will print all packets, making protocol debugging easier for those who are interested.

### Opmodes (the first is the basic one that just hands control over to blazeftc rust, the second shows how to run an opmode on top of the neutrino proxy):

### Default to hand over control:
```kotlin
import com.bylazar.telemetry.JoinedTelemetry
import com.bylazar.telemetry.PanelsTelemetry
import com.qualcomm.robotcore.eventloop.opmode.TeleOp
import com.qualcomm.robotcore.util.ElapsedTime

@TeleOp(name = "BlazeFTC")
//@Configurable //put whatever configuration annotations you need
class DummyPlug : DummyPlugOpMode() {
    //@Configurable
    companion object {
        @JvmStatic
        var toRun = 1
        @JvmStatic
        var millisToWait = 5L
        @JvmStatic
        var nanosToWait = 0
    }

    override fun runOpMode() {
        //this function can be called after waitForStart but like, don't do that
        //once you've supplied it telemetry, just use the "telemetry" variable it has been set
        initializeBlazeFTC(JoinedTelemetry(telemetry, PanelsTelemetry.ftcTelemetry))

        waitForStart()

        runBlazeFTC(1)//always call this please. toRun is an integer passed into the rust code you can control

        val timer = ElapsedTime()
        while (!isStopRequested) {
            updateGamepads()//*probably* need to call this. depends on what you're doing

            val ms = timer.milliseconds()//timer. feel free to remove this
            timer.reset()
            telemetry.addData("java loop time", "$ms[ms]")
            //the thread sleep is so you don't send so many packets you crash the system
            //there is probably a better way to do it than this but I don't know what it is.
            Thread.sleep(millisToWait, nanosToWait)
        }
    }
}
```


### Neutrino Proxy Stub:
```kotlin
import com.bylazar.telemetry.JoinedTelemetry
import com.bylazar.telemetry.PanelsTelemetry
import com.qualcomm.robotcore.eventloop.opmode.TeleOp
import com.qualcomm.robotcore.hardware.DcMotorEx
import com.qualcomm.robotcore.hardware.DcMotorSimple
import com.qualcomm.robotcore.hardware.Gamepad
import com.qualcomm.robotcore.util.ElapsedTime
import org.firstinspires.ftc.robotcore.external.Telemetry

@TeleOp(name = "NeutrinoTest")
//@Configurable //replace with whatever configurables library you use
class NeutrinoTest : DummyPlugOpMode() {
    //@Configurable
    companion object {
        //feel free to decrease these, but note: if the "saturation" log in telemetry (you'll see it)
        //reaches 100%, the system will crash. No, I have not fixed it yet.
        @JvmStatic
        var millisToWait = 5L
        @JvmStatic
        var nanosToWait = 0
    }

    override fun runOpMode() {
        //pass this function a telemetry, then use the opmode's telemetry after that.
        initializeBlazeFTC(JoinedTelemetry(telemetry, PanelsTelemetry.ftcTelemetry))
        //this mecanum code is just an example. do whatever you want here:
        val mecanum = Mecanum(hardwareMap.get(DcMotorEx::class.java, "flMotor"),
            hardwareMap.get(DcMotorEx::class.java, "frMotor"),
            hardwareMap.get(DcMotorEx::class.java, "blMotor"),
            hardwareMap.get(DcMotorEx::class.java, "brMotor"))
        waitForStart()
        runBlazeFTC(0)//call this and pass 0 to start the default neutrino handler
        val time = ElapsedTime()
        while (!isStopRequested) {
            mecanum.mecanumLoop(gamepad1)//call whatever you want, your code goes here!
            mecanum.telemetry(telemetry)
            //updateGamepads()//no need for this because neutrino does not use them
            val ms = time.milliseconds()
            telemetry.addData("java loop time", "${ms - millisToWait - nanosToWait / 999999}[ms]")
            time.reset()
            Thread.sleep(millisToWait, nanosToWait)
        }
    }
    class Mecanum(private val flMotor: DcMotorEx,
                  private val frMotor: DcMotorEx,
                  private val blMotor: DcMotorEx,
                  private val brMotor: DcMotorEx) {

        init {
            blMotor.direction = DcMotorSimple.Direction.FORWARD
            flMotor.direction = DcMotorSimple.Direction.FORWARD
            frMotor.direction = DcMotorSimple.Direction.REVERSE
            brMotor.direction = DcMotorSimple.Direction.REVERSE
        }

        fun telemetry(telemetry: Telemetry) {
            telemetry.addData("Front Left Power", flMotor.power)
            telemetry.addData("Front Right Power", frMotor.power)
            telemetry.addData("Back Left Power", blMotor.power)
            telemetry.addData("Back Right Power", brMotor.power)
        }

        fun mecanumLoop(gamepad1: Gamepad){
            val y = gamepad1.left_stick_y.toDouble()
            val x = -gamepad1.left_stick_x.toDouble()
            val turn = -gamepad1.right_stick_x.toDouble()

            flMotor.power = (y + x + turn)
            blMotor.power = (y - x + turn)
            frMotor.power = (y - x - turn)
            brMotor.power = (y + x - turn)
        }
    }
}
```