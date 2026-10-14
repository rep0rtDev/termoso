package com.termoso.android.data.serial

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.os.Build
import com.termoso.core.SerialLine
import java.io.IOException
import kotlin.coroutines.resume
import kotlinx.coroutines.suspendCancellableCoroutine

/**
 * USB-UART adapters driven through `android.hardware.usb`, no native code and no
 * extra dependency: CDC-ACM (Arduino, STM32, most "virtual COM" boards), FTDI
 * FT232/FT2232/FT4232/FT232H/FT231X, Silicon Labs CP210x and WCH CH340/CH341/CH9102.
 *
 * Nothing here talks to the network; the bytes go straight between the adapter
 * and the terminal in Rust.
 */
object UsbSerial {
    /** An attached adapter a driver knows how to talk to. */
    data class Adapter(
        val device: UsbDevice,
        val driver: Driver,
        /** Interface the port lives on (FTDI multi-port chips expose several). */
        val port: Int,
    ) {
        val name: String
            get() = device.productName?.takeIf { it.isNotBlank() }
                ?: "%s %04x:%04x".format(driver.label, device.vendorId, device.productId)

        /** Stable-enough identity for history rows and reconnects. */
        val key: String get() = "${device.deviceName}#$port"
    }

    enum class Driver(val label: String) {
        CDC_ACM("CDC-ACM"),
        FTDI("FTDI"),
        CP210X("CP210x"),
        CH34X("CH34x"),
    }

    private const val VID_FTDI = 0x0403
    private const val VID_SILABS = 0x10C4
    private const val VID_WCH = 0x1A86

    private val FTDI_PIDS = setOf(0x6001, 0x6010, 0x6011, 0x6014, 0x6015)
    private val SILABS_PIDS = setOf(0xEA60, 0xEA70, 0xEA71, 0xEA80)
    private val WCH_PIDS = setOf(0x7523, 0x7522, 0x5523, 0x55D4, 0xE523)

    /** Which driver, if any, handles [device]. */
    fun driverFor(device: UsbDevice): Driver? = when {
        device.vendorId == VID_FTDI && device.productId in FTDI_PIDS -> Driver.FTDI
        device.vendorId == VID_SILABS && device.productId in SILABS_PIDS -> Driver.CP210X
        device.vendorId == VID_WCH && device.productId in WCH_PIDS -> Driver.CH34X
        hasCdcAcm(device) -> Driver.CDC_ACM
        else -> null
    }

    private fun hasCdcAcm(device: UsbDevice): Boolean =
        (0 until device.interfaceCount).any { i ->
            val itf = device.getInterface(i)
            itf.interfaceClass == UsbConstants.USB_CLASS_COMM && itf.interfaceSubclass == 2
        }

    /** Every supported adapter currently plugged in, one entry per port. */
    fun adapters(context: Context): List<Adapter> {
        val usb = context.getSystemService(Context.USB_SERVICE) as? UsbManager ?: return emptyList()
        return usb.deviceList.values.flatMap { adaptersOf(it) }.sortedBy { it.name }
    }

    fun adaptersOf(device: UsbDevice): List<Adapter> {
        val driver = driverFor(device) ?: return emptyList()
        val ports = when (driver) {
            Driver.FTDI -> device.interfaceCount.coerceAtLeast(1)
            Driver.CP210X -> device.interfaceCount.coerceAtLeast(1)
            Driver.CDC_ACM -> (0 until device.interfaceCount).count { i ->
                device.getInterface(i).interfaceClass == UsbConstants.USB_CLASS_COMM
            }.coerceAtLeast(1)
            Driver.CH34X -> 1
        }
        return (0 until ports).map { Adapter(device, driver, it) }
    }

    fun find(context: Context, key: String): Adapter? = adapters(context).firstOrNull { it.key == key }

    /** Ask the system for access to [device]; `true` once the user allowed it. */
    suspend fun requestPermission(context: Context, device: UsbDevice): Boolean {
        val usb = context.getSystemService(Context.USB_SERVICE) as? UsbManager ?: return false
        if (usb.hasPermission(device)) return true
        val action = "${context.packageName}.USB_PERMISSION.${System.nanoTime()}"
        return suspendCancellableCoroutine { cont ->
            val receiver = object : BroadcastReceiver() {
                override fun onReceive(ctx: Context, intent: Intent) {
                    if (intent.action != action) return
                    runCatching { ctx.unregisterReceiver(this) }
                    val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                    if (cont.isActive) cont.resume(granted)
                }
            }
            val filter = IntentFilter(action)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
            } else {
                @Suppress("UnspecifiedRegisterReceiverFlag")
                context.registerReceiver(receiver, filter)
            }
            val flags = PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_ONE_SHOT
            val pi = PendingIntent.getBroadcast(
                context,
                0,
                Intent(action).setPackage(context.packageName),
                flags,
            )
            usb.requestPermission(device, pi)
            cont.invokeOnCancellation { runCatching { context.unregisterReceiver(receiver) } }
        }
    }

    /** Open [adapter] with [line]; the caller owns the returned port. */
    @Throws(IOException::class)
    fun open(context: Context, adapter: Adapter, line: SerialLine): Port {
        val usb = context.getSystemService(Context.USB_SERVICE) as? UsbManager
            ?: throw IOException("USB host not available")
        val conn = usb.openDevice(adapter.device) ?: throw IOException("Could not open ${adapter.name}")
        return try {
            when (adapter.driver) {
                Driver.CDC_ACM -> CdcAcmPort(adapter, conn)
                Driver.FTDI -> FtdiPort(adapter, conn)
                Driver.CP210X -> Cp210xPort(adapter, conn)
                Driver.CH34X -> Ch34xPort(adapter, conn)
            }.also { it.configure(line) }
        } catch (e: Throwable) {
            conn.close()
            throw e
        }
    }

    /** One open UART. Reads block for at most [READ_TIMEOUT_MS] so [close] is noticed. */
    abstract class Port(protected val adapter: Adapter, protected val conn: UsbDeviceConnection) {
        protected lateinit var dataInterface: UsbInterface
        protected lateinit var endpointIn: UsbEndpoint
        protected lateinit var endpointOut: UsbEndpoint
        protected val claimed = mutableListOf<UsbInterface>()

        @Volatile
        var isOpen = true
            private set

        protected fun claim(itf: UsbInterface) {
            if (!conn.claimInterface(itf, true)) throw IOException("Could not claim USB interface ${itf.id}")
            claimed += itf
        }

        /** Bulk IN/OUT pair of [itf]. */
        protected fun bulkEndpoints(itf: UsbInterface) {
            var inEp: UsbEndpoint? = null
            var outEp: UsbEndpoint? = null
            for (i in 0 until itf.endpointCount) {
                val ep = itf.getEndpoint(i)
                if (ep.type != UsbConstants.USB_ENDPOINT_XFER_BULK) continue
                if (ep.direction == UsbConstants.USB_DIR_IN) inEp = inEp ?: ep else outEp = outEp ?: ep
            }
            endpointIn = inEp ?: throw IOException("No bulk IN endpoint")
            endpointOut = outEp ?: throw IOException("No bulk OUT endpoint")
            dataInterface = itf
        }

        protected fun controlOut(type: Int, request: Int, value: Int, index: Int, data: ByteArray? = null) {
            val n = conn.controlTransfer(type, request, value, index, data, data?.size ?: 0, CONTROL_TIMEOUT_MS)
            if (n < 0) throw IOException("USB control request 0x%02x failed".format(request))
        }

        abstract fun configure(line: SerialLine)

        /** Bytes read, or `null` when the port is gone. Empty array = nothing yet. */
        open fun read(buffer: ByteArray): ByteArray? {
            if (!isOpen) return null
            val n = conn.bulkTransfer(endpointIn, buffer, buffer.size, READ_TIMEOUT_MS)
            return when {
                n > 0 -> buffer.copyOf(n)
                n == 0 || n == -1 && isOpen -> ByteArray(0)
                else -> null
            }
        }

        fun write(data: ByteArray) {
            var off = 0
            while (off < data.size && isOpen) {
                val len = minOf(data.size - off, endpointOut.maxPacketSize.coerceAtLeast(64))
                val chunk = if (off == 0 && len == data.size) data else data.copyOfRange(off, off + len)
                val n = conn.bulkTransfer(endpointOut, chunk, len, WRITE_TIMEOUT_MS)
                if (n < 0) throw IOException("USB write failed")
                off += n.coerceAtLeast(1)
            }
        }

        open fun close() {
            if (!isOpen) return
            isOpen = false
            claimed.forEach { runCatching { conn.releaseInterface(it) } }
            runCatching { conn.close() }
        }

        companion object {
            const val READ_TIMEOUT_MS = 200
            const val WRITE_TIMEOUT_MS = 2000
            const val CONTROL_TIMEOUT_MS = 2000
        }
    }

    /** USB CDC ACM: a control interface (class 2/2) plus a data interface (class 10). */
    private class CdcAcmPort(adapter: Adapter, conn: UsbDeviceConnection) : Port(adapter, conn) {
        private val control: UsbInterface

        init {
            val device = adapter.device
            val controls = (0 until device.interfaceCount).map { device.getInterface(it) }
                .filter { it.interfaceClass == UsbConstants.USB_CLASS_COMM }
            control = controls.getOrNull(adapter.port) ?: controls.firstOrNull() ?: device.getInterface(0)
            claim(control)
            val data = (0 until device.interfaceCount).map { device.getInterface(it) }
                .filter { it.interfaceClass == UsbConstants.USB_CLASS_CDC_DATA }
                .getOrNull(adapter.port)
            if (data != null) {
                claim(data)
                bulkEndpoints(data)
            } else {
                bulkEndpoints(control)
            }
        }

        override fun configure(line: SerialLine) {
            val stop = if (line.stopBits.toInt() == 2) 2 else 0
            val parity = when (line.parity) { "odd" -> 1; "even" -> 2; "mark" -> 3; "space" -> 4; else -> 0 }
            val baud = line.baudRate.toLong()
            val coding = byteArrayOf(
                (baud and 0xff).toByte(), (baud shr 8 and 0xff).toByte(),
                (baud shr 16 and 0xff).toByte(), (baud shr 24 and 0xff).toByte(),
                stop.toByte(), parity.toByte(), line.dataBits.toByte(),
            )
            controlOut(REQ_OUT_CLASS, SET_LINE_CODING, 0, control.id, coding)
            // DTR + RTS so the far side sees us as present.
            controlOut(REQ_OUT_CLASS, SET_CONTROL_LINE_STATE, 0x03, control.id)
        }

        companion object {
            const val REQ_OUT_CLASS = 0x21
            const val SET_LINE_CODING = 0x20
            const val SET_CONTROL_LINE_STATE = 0x22
        }
    }

    /** FTDI: vendor requests; every IN packet starts with two modem-status bytes. */
    private class FtdiPort(adapter: Adapter, conn: UsbDeviceConnection) : Port(adapter, conn) {
        private val index: Int

        init {
            val device = adapter.device
            val itf = device.getInterface(adapter.port.coerceIn(0, device.interfaceCount - 1))
            claim(itf)
            bulkEndpoints(itf)
            index = if (device.interfaceCount > 1) adapter.port + 1 else 0
        }

        override fun configure(line: SerialLine) {
            controlOut(REQ_OUT_VENDOR, SIO_RESET, 0, index)
            setBaud(line.baudRate.toInt())
            val parity = when (line.parity) { "odd" -> 1; "even" -> 2; "mark" -> 3; "space" -> 4; else -> 0 }
            val stop = if (line.stopBits.toInt() == 2) 2 else 0
            controlOut(REQ_OUT_VENDOR, SIO_SET_DATA, line.dataBits.toInt() or (parity shl 8) or (stop shl 11), index)
            val flow = when (line.flowControl) { "hardware" -> 0x0100; "software" -> 0x0400; else -> 0 }
            controlOut(REQ_OUT_VENDOR, SIO_SET_FLOW_CTRL, if (flow == 0x0400) 0x1311 else 0, flow or index)
            controlOut(REQ_OUT_VENDOR, SIO_MODEM_CTRL, 0x0101 or 0x0202, index)
        }

        private fun setBaud(baud: Int) {
            if (baud <= 0) throw IOException("Bad baud rate $baud")
            var divisor: Int
            val sub: Int
            when {
                baud >= 2_500_000 -> { divisor = 0; sub = 0 }
                baud >= 1_750_000 -> { divisor = 1; sub = 0 }
                else -> {
                    divisor = ((24_000_000 shl 1) / baud + 1) shr 1
                    sub = divisor and 0x07
                    divisor = divisor shr 3
                    if (divisor > 0x3fff) throw IOException("Baud rate $baud too low for FTDI")
                }
            }
            var value = divisor
            var idx = 0
            when (sub) {
                1 -> value = value or 0xc000
                2 -> value = value or 0x8000
                3 -> idx = 1
                4 -> value = value or 0x4000
                5 -> { value = value or 0x4000; idx = 1 }
                6 -> { value = value or 0x8000; idx = 1 }
                7 -> { value = value or 0xc000; idx = 1 }
            }
            val wIndex = if (adapter.device.interfaceCount > 1) (idx shl 8) or index else idx
            controlOut(REQ_OUT_VENDOR, SIO_SET_BAUD_RATE, value, wIndex)
        }

        override fun read(buffer: ByteArray): ByteArray? {
            if (!isOpen) return null
            val n = conn.bulkTransfer(endpointIn, buffer, buffer.size, READ_TIMEOUT_MS)
            if (n < 0) return if (isOpen) ByteArray(0) else null
            if (n <= 2) return ByteArray(0)
            val packet = endpointIn.maxPacketSize.coerceAtLeast(64)
            val out = java.io.ByteArrayOutputStream(n)
            var off = 0
            while (off < n) {
                val len = minOf(packet, n - off)
                if (len > 2) out.write(buffer, off + 2, len - 2)
                off += len
            }
            return out.toByteArray()
        }

        companion object {
            const val REQ_OUT_VENDOR = 0x40
            const val SIO_RESET = 0x00
            const val SIO_MODEM_CTRL = 0x01
            const val SIO_SET_FLOW_CTRL = 0x02
            const val SIO_SET_BAUD_RATE = 0x03
            const val SIO_SET_DATA = 0x04
        }
    }

    /** Silicon Labs CP210x. */
    private class Cp210xPort(adapter: Adapter, conn: UsbDeviceConnection) : Port(adapter, conn) {
        private val index: Int

        init {
            val device = adapter.device
            val itf = device.getInterface(adapter.port.coerceIn(0, device.interfaceCount - 1))
            claim(itf)
            bulkEndpoints(itf)
            index = itf.id
        }

        override fun configure(line: SerialLine) {
            controlOut(REQ_OUT_VENDOR, IFC_ENABLE, 1, index)
            val baud = line.baudRate.toLong()
            controlOut(
                REQ_OUT_VENDOR, SET_BAUDRATE, 0, index,
                byteArrayOf(
                    (baud and 0xff).toByte(), (baud shr 8 and 0xff).toByte(),
                    (baud shr 16 and 0xff).toByte(), (baud shr 24 and 0xff).toByte(),
                ),
            )
            val parity = when (line.parity) { "odd" -> 1; "even" -> 2; "mark" -> 3; "space" -> 4; else -> 0 }
            val stop = if (line.stopBits.toInt() == 2) 2 else 0
            controlOut(REQ_OUT_VENDOR, SET_LINE_CTL, (line.dataBits.toInt() shl 8) or (parity shl 4) or stop, index)
            // ulControlHandshake, ulFlowReplace, ulXonLimit, ulXoffLimit (little-endian u32 each).
            val handshake = when (line.flowControl) { "hardware" -> 0x09; else -> 0x01 }
            val replace = when (line.flowControl) { "hardware" -> 0x80; "software" -> 0x43; else -> 0x40 }
            controlOut(
                REQ_OUT_VENDOR, SET_FLOW, 0, index,
                byteArrayOf(
                    handshake.toByte(), 0, 0, 0,
                    replace.toByte(), 0, 0, 0,
                    0, 0, 0, 0,
                    0, 0, 0, 0,
                ),
            )
            controlOut(REQ_OUT_VENDOR, SET_MHS, 0x0101 or 0x0202, index)
        }

        override fun close() {
            if (isOpen) runCatching { controlOut(REQ_OUT_VENDOR, IFC_ENABLE, 0, index) }
            super.close()
        }

        companion object {
            const val REQ_OUT_VENDOR = 0x41
            const val IFC_ENABLE = 0x00
            const val SET_LINE_CTL = 0x03
            const val SET_MHS = 0x07
            const val SET_FLOW = 0x13
            const val SET_BAUDRATE = 0x1E
        }
    }

    /** WCH CH340 / CH341 / CH9102. */
    private class Ch34xPort(adapter: Adapter, conn: UsbDeviceConnection) : Port(adapter, conn) {
        init {
            val device = adapter.device
            val itf = (0 until device.interfaceCount).map { device.getInterface(it) }
                .firstOrNull { itf -> (0 until itf.endpointCount).any { itf.getEndpoint(it).type == UsbConstants.USB_ENDPOINT_XFER_BULK } }
                ?: device.getInterface(0)
            claim(itf)
            bulkEndpoints(itf)
        }

        override fun configure(line: SerialLine) {
            controlOut(REQ_OUT_VENDOR, CMD_SERIAL_INIT, 0, 0)
            setBaud(line.baudRate.toInt())
            var lcr = LCR_ENABLE_RX or LCR_ENABLE_TX or (line.dataBits.toInt() - 5).coerceIn(0, 3)
            if (line.stopBits.toInt() == 2) lcr = lcr or LCR_STOP_BITS_2
            lcr = lcr or when (line.parity) {
                "odd" -> LCR_ENABLE_PAR
                "even" -> LCR_ENABLE_PAR or LCR_PAR_EVEN
                "mark" -> LCR_ENABLE_PAR or LCR_MARK_SPACE
                "space" -> LCR_ENABLE_PAR or LCR_MARK_SPACE or LCR_PAR_EVEN
                else -> 0
            }
            controlOut(REQ_OUT_VENDOR, CMD_REG_WRITE, REG_LCR, lcr)
            controlOut(REQ_OUT_VENDOR, CMD_REG_WRITE, REG_FLOW, if (line.flowControl == "hardware") 0x0101 else 0)
            // Active-low DTR (0x20) and RTS (0x40).
            controlOut(REQ_OUT_VENDOR, CMD_MODEM_CTRL, (0x20 or 0x40).inv() and 0xff, 0)
        }

        private fun setBaud(baud: Int) {
            if (baud <= 0) throw IOException("Bad baud rate $baud")
            var factor: Long
            var divisor: Long
            if (baud == 921_600) {
                divisor = 7
                factor = 0xf300
            } else {
                factor = BAUDBASE_FACTOR / baud
                divisor = BAUDBASE_DIVMAX
                while (factor > 0xfff0 && divisor > 0) {
                    factor = factor shr 3
                    divisor--
                }
                if (factor > 0xfff0) throw IOException("Baud rate $baud too low for CH34x")
                factor = 0x10000 - factor
            }
            divisor = divisor or 0x0080
            controlOut(REQ_OUT_VENDOR, CMD_REG_WRITE, REG_BAUD, ((factor and 0xff00) or divisor).toInt())
            controlOut(REQ_OUT_VENDOR, CMD_REG_WRITE, REG_BAUD_FACTOR, (factor and 0xff).toInt())
        }

        companion object {
            const val REQ_OUT_VENDOR = 0x40
            const val CMD_REG_WRITE = 0x9A
            const val CMD_SERIAL_INIT = 0xA1
            const val CMD_MODEM_CTRL = 0xA4
            const val REG_BAUD = 0x1312
            const val REG_BAUD_FACTOR = 0x0F2C
            const val REG_LCR = 0x2518
            const val REG_FLOW = 0x2727
            const val LCR_ENABLE_RX = 0x80
            const val LCR_ENABLE_TX = 0x40
            const val LCR_MARK_SPACE = 0x20
            const val LCR_PAR_EVEN = 0x10
            const val LCR_ENABLE_PAR = 0x08
            const val LCR_STOP_BITS_2 = 0x04
            const val BAUDBASE_FACTOR = 1_532_620_800L
            const val BAUDBASE_DIVMAX = 3L
        }
    }
}
