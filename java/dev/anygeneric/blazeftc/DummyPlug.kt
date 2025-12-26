package dev.anygeneric.blazeftc

import com.bylazar.configurables.annotations.Configurable
import com.bylazar.telemetry.JoinedTelemetry
import com.bylazar.telemetry.PanelsTelemetry
import com.qualcomm.hardware.lynx.LynxModule
import com.qualcomm.hardware.lynx.LynxUsbDevice
import com.qualcomm.robotcore.eventloop.opmode.LinearOpMode
import com.qualcomm.robotcore.eventloop.opmode.TeleOp
import com.qualcomm.robotcore.hardware.usb.serial.RobotUsbDeviceTty
import com.qualcomm.robotcore.hardware.usb.serial.SerialPort
import com.qualcomm.robotcore.util.ElapsedTime
import org.firstinspires.ftc.robotcore.external.navigation.VoltageUnit
import java.io.File
import java.io.FileDescriptor
import java.io.FileInputStream
import java.io.FileOutputStream

@TeleOp(name = "BlazeFTC")
@Configurable
class DummyPlug : LinearOpMode() {
    @Configurable
    companion object {
        @JvmStatic
        var speed = 4
        @JvmStatic
        var nanos = 1.0
        @JvmStatic
        var toRun = 2
    }
    var opened = false
    var fd: FileDescriptor? = null
    var kill: () -> Unit = {}
    var elapsedTime = ElapsedTime()
    override fun runOpMode() {
        val module = hardwareMap.getAll(LynxModule::class.java)
        module.forEach { it.bulkCachingMode = LynxModule.BulkCachingMode.MANUAL; it.clearBulkCache() }
        module.forEach { println("MODULE ADDRESS: " + it.moduleAddress + ": " + it.isParent) }
        val usbDevField = LynxModule::class.java.getDeclaredField("lynxUsbDevice").also { it.isAccessible = true }
        val serialPortField = RobotUsbDeviceTty::class.java.getDeclaredField("serialPort").also { it.isAccessible = true }
        val fileDescriptorField = SerialPort::class.java.getDeclaredField("fileDescriptor").also { it.isAccessible = true }
        val inputStreamField = SerialPort::class.java.getDeclaredField("fileInputStream").also { it.isAccessible = true }
        val outputStreamField = SerialPort::class.java.getDeclaredField("fileOutputStream").also { it.isAccessible = true }
        val fileField = SerialPort::class.java.getDeclaredField("file").also { it.isAccessible = true }
        module.first().also {
            val device = usbDevField.get(it)
            if (device != null && device is LynxUsbDevice) {
                val port = serialPortField.get(device.robotUsbDevice) as SerialPort
                fd = fileDescriptorField.get(port) as FileDescriptor
                val oldInputStream = inputStreamField.get(port) as FileInputStream
                val oldOutputStream = outputStreamField.get(port) as FileOutputStream
                kill = {
                    inputStreamField.set(port, oldInputStream)
                    outputStreamField.set(port, oldOutputStream)
                }
                val file = fileField.get(port) as File
                val x = this
                val nullOutputStream = FileOutputStream("/dev/null")
                val nullFd = nullOutputStream.fd
                val fakeFileOutputStream = object : FileOutputStream(nullFd) {
                    override fun write(b: ByteArray?, off: Int, len: Int) {
                        //println("write called! len=$len, opened:$opened")
                        if (!opened)
                            open(file, oldInputStream, oldOutputStream)
                        if (b == null)
                            return
                        BlazeFTC.write(b.slice(off..<off+len).toByteArray())
                    }
                }
                val fakeFileInputStream = object : FileInputStream(nullFd) {
                    override fun read(b: ByteArray?, off: Int, len: Int): Int {
                        //println("read called! len:$len, opened:$opened")
                        if (!opened)
                            open(file, oldInputStream, oldOutputStream)
                        return BlazeFTC.read(b!!, off, len)
                    }
                    /*override fun available(): Int {
                        if (!opened)
                            open(file, oldInputStream, oldOutputStream)
                        return BlazeFTC.available()
                    }*/
                    // /dev/ttyHS4
                }
                module.first().getInputVoltage(VoltageUnit.VOLTS)//force a command to be sent, causing the wait thread to use our fake stream
                println("ready to call IS set")
                inputStreamField.set(port, fakeFileInputStream)//set input stream, will be used next time stream is grabbed to block on
                println("set input stream. calling get input voltage")
                println("java: voltage0: " + module.first().getInputVoltage(VoltageUnit.VOLTS))//force a command to be sent, causing the wait thread to use our fake stream
                outputStreamField.set(port, fakeFileOutputStream)//now set our stream as default out
                println("closing old streams...")
                //oldOutputStream.close()//TODO calling this seems to break things. maybe don't?
                //oldInputStream.close()
                println("actually did call get input voltage")
                println("java: voltage1: " + module.first().getInputVoltage(VoltageUnit.VOLTS))//ok now send a command through our fake infrastructure
                println("got second voltage.")
            }
        }
        module.forEach {
            println("informing of module: " + it.moduleAddress + ": " + it.isParent)
            BlazeFTC.informOfModule(it.moduleAddress, it.isParent)
        }
        waitForStart()
        BlazeFTC.run(toRun)
        elapsedTime.reset()
        try {
            while (!isStopRequested) {
                if (speed != 0) {
                    Thread.sleep(speed.toLong(), (nanos * 999999).toInt())
                }
                BlazeFTC.gamepad(gamepad1.toByteArray()!!, gamepad2.toByteArray()!!)
            }
        } finally {
            kill()//put the streams back idk this doesn't work haha
            BlazeFTC.close()
        }
    }
    fun open(file: File, oldInputStream: FileInputStream, oldOutputStream: FileOutputStream) {
        val joined = JoinedTelemetry(telemetry, PanelsTelemetry.ftcTelemetry)
        BlazeFTC.openFile(fd, BlazeFTC.BlazeTelemetry(joined))
        opened = true
    }
}