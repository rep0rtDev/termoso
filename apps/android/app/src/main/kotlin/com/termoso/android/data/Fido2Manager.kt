package com.termoso.android.data

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.nfc.NfcAdapter
import android.nfc.Tag
import android.nfc.TagLostException
import android.nfc.tech.IsoDep
import android.os.Build
import android.os.Bundle
import android.util.Log
import com.termoso.core.Fido2DeviceCard
import com.termoso.core.Fido2Devices
import com.termoso.core.Fido2NfcLink
import com.termoso.core.Fido2UsbLink
import com.termoso.core.MobileException
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Android side of FIDO2 security keys: finds CTAPHID tokens on the USB host
 * port and CTAP2 tokens over NFC, opens the raw pipes and hands them to the
 * Rust registry ([Fido2Devices]). Everything above the wire — CTAPHID/APDU
 * framing, CTAP2, PIN protocol, credentials, SSH `sk-*` signatures — lives in
 * Rust; Kotlin never sees a PIN, a credential or a signature.
 *
 * Process-wide (hardware is not vault-scoped). USB tokens stay attached while
 * plugged in; an NFC token is attached while it is held to the phone and reader
 * mode is on, which screens turn on only while they need it.
 */
class Fido2Manager(context: Context) {
    private val app = context.applicationContext
    private val usb: UsbManager? = app.getSystemService(UsbManager::class.java)
    private val nfc: NfcAdapter? = runCatching { NfcAdapter.getDefaultAdapter(app) }.getOrNull()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** Rust-owned registry; shared with the connection path (`PhoneBackend`). */
    val registry: Fido2Devices = Fido2Devices.shared()

    /** True when the device can act as a USB host at all. */
    val usbHost: Boolean = app.packageManager.hasSystemFeature(PackageManager.FEATURE_USB_HOST)

    /** True when the device has an NFC reader (it may still be switched off). */
    val nfcHardware: Boolean = nfc != null

    val nfcEnabled: Boolean get() = nfc?.isEnabled == true

    private val _devices = MutableStateFlow<List<Fido2DeviceCard>>(emptyList())

    /** Tokens currently attached, as Rust reports them. */
    val devices: StateFlow<List<Fido2DeviceCard>> = _devices.asStateFlow()

    private val _usbPending = MutableStateFlow(0)

    /** USB devices that look like a token and are waiting for the permission dialog. */
    val usbPending: StateFlow<Int> = _usbPending.asStateFlow()

    private val links = mutableMapOf<String, AutoCloseable>()
    private val asked = mutableSetOf<String>()
    private var nfcUsers = 0
    private var nfcWatch: Job? = null
    private var started = false

    private val usbEvents = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val device: UsbDevice? = intent.getParcelableExtraCompat(UsbManager.EXTRA_DEVICE)
            when (intent.action) {
                UsbManager.ACTION_USB_DEVICE_ATTACHED -> device?.let { scope.launch { consider(it) } }
                UsbManager.ACTION_USB_DEVICE_DETACHED -> device?.let { detach(usbId(it)) }
                ACTION_USB_PERMISSION -> {
                    _usbPending.value = (_usbPending.value - 1).coerceAtLeast(0)
                    if (device != null && intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)) {
                        scope.launch { attachUsb(device) }
                    }
                }
            }
        }
    }

    /** Register for plug/unplug and look at what is already connected. */
    fun start() {
        if (started) return
        started = true
        if (usb != null && usbHost) {
            val system = IntentFilter().apply {
                addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
                addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            }
            registerReceiverCompat(system, exported = true)
            registerReceiverCompat(IntentFilter(ACTION_USB_PERMISSION), exported = false)
            refreshUsb()
        }
    }

    /** Re-scan the USB bus; asks permission for tokens the user has not approved yet. */
    fun refreshUsb() {
        val manager = usb ?: return
        scope.launch { manager.deviceList.values.forEach { consider(it) } }
    }

    /**
     * Start listening for NFC tokens while [activity] is in front. Balanced by
     * [disableNfc]; several screens may hold the reader at once.
     */
    fun enableNfc(activity: Activity) {
        val adapter = nfc ?: return
        nfcUsers += 1
        if (nfcUsers > 1) return
        val extras = Bundle().apply { putInt(NfcAdapter.EXTRA_READER_PRESENCE_CHECK_DELAY, 250) }
        runCatching {
            adapter.enableReaderMode(
                activity,
                { tag -> scope.launch { attachNfc(tag) } },
                NfcAdapter.FLAG_READER_NFC_A or NfcAdapter.FLAG_READER_NFC_B or
                    NfcAdapter.FLAG_READER_SKIP_NDEF_CHECK or NfcAdapter.FLAG_READER_NO_PLATFORM_SOUNDS,
                extras,
            )
        }.onFailure { Log.w(TAG, "reader mode", it) }
    }

    fun disableNfc(activity: Activity) {
        val adapter = nfc ?: return
        if (nfcUsers == 0) return
        nfcUsers -= 1
        if (nfcUsers > 0) return
        runCatching { adapter.disableReaderMode(activity) }
        synchronized(links) { links.keys.filter { it.startsWith("nfc:") } }.forEach(::detach)
    }

    /** Drop one token (e.g. after a link error); a plugged USB token comes back on [refreshUsb]. */
    fun detach(id: String) {
        val link = synchronized(links) { links.remove(id) }
        synchronized(asked) { asked.remove(id) }
        registry.detach(id)
        runCatching { link?.close() }
        publish()
    }

    fun close() {
        synchronized(links) { links.keys.toList() }.forEach(::detach)
        registry.detachAll()
        if (started) runCatching { app.unregisterReceiver(usbEvents) }
        started = false
    }

    private fun publish() {
        _devices.value = registry.list()
    }

    // ---- USB ----------------------------------------------------------------

    private fun consider(device: UsbDevice) {
        val manager = usb ?: return
        if (ctapInterface(device) == null) return
        val id = usbId(device)
        if (synchronized(links) { id in links }) return
        if (manager.hasPermission(device)) {
            attachUsb(device)
            return
        }
        if (!synchronized(asked) { asked.add(id) }) return
        _usbPending.value += 1
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or
            (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
        val intent = Intent(ACTION_USB_PERMISSION).setPackage(app.packageName)
        manager.requestPermission(device, PendingIntent.getBroadcast(app, 0, intent, flags))
    }

    private fun attachUsb(device: UsbDevice) {
        val manager = usb ?: return
        val id = usbId(device)
        if (synchronized(links) { id in links }) return
        val iface = ctapInterface(device) ?: return
        val conn = manager.openDevice(device) ?: run {
            Log.w(TAG, "openDevice failed for ${device.deviceName}")
            return
        }
        if (!conn.claimInterface(iface, true)) {
            conn.close()
            return
        }
        if (!isFidoHid(conn, iface)) {
            conn.releaseInterface(iface)
            conn.close()
            return
        }
        val link = UsbHidLink(conn, iface)
        synchronized(links) { links[id] = link }
        runCatching {
            registry.attachUsb(
                id,
                device.productName ?: "USB security key",
                device.vendorId.toUShort(),
                device.productId.toUShort(),
                link,
            )
        }.onFailure {
            Log.w(TAG, "attach ${device.deviceName}: ${it.userMessage()}")
            synchronized(links) { links.remove(id) }
            link.close()
        }
        publish()
    }

    // ---- NFC ----------------------------------------------------------------

    private fun attachNfc(tag: Tag) {
        val iso = IsoDep.get(tag) ?: return
        val id = "nfc:" + tag.id.joinToString("") { "%02x".format(it) }
        synchronized(links) { links.keys.filter { it.startsWith("nfc:") } }.forEach(::detach)
        try {
            iso.connect()
            iso.timeout = 5_000
        } catch (e: IOException) {
            Log.w(TAG, "nfc connect", e)
            return
        }
        val link = NfcIsoDepLink(iso)
        synchronized(links) { links[id] = link }
        val attached = runCatching { registry.attachNfc(id, link) }
            .onFailure {
                Log.w(TAG, "nfc attach: ${it.userMessage()}")
                synchronized(links) { links.remove(id) }
                link.close()
            }
        publish()
        if (attached.isFailure) return
        nfcWatch?.cancel()
        nfcWatch = scope.launch {
            while (isActive && iso.isConnected) delay(500)
            if (synchronized(links) { links[id] === link }) detach(id)
        }
    }

    private fun registerReceiverCompat(filter: IntentFilter, exported: Boolean) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val flag = if (exported) Context.RECEIVER_EXPORTED else Context.RECEIVER_NOT_EXPORTED
            app.registerReceiver(usbEvents, filter, flag)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            app.registerReceiver(usbEvents, filter)
        }
    }

    private companion object {
        const val TAG = "Fido2"
        const val ACTION_USB_PERMISSION = "com.termoso.android.USB_PERMISSION"
    }
}

private fun usbId(device: UsbDevice) = "usb:${device.deviceName}"

/**
 * The CTAPHID interface of [device], if it has one: a HID interface that is
 * not a boot keyboard/mouse and carries one 64-byte interrupt pipe each way.
 * Whether it really speaks FIDO is confirmed from the report descriptor once
 * the device is open ([isFidoHid]); this pre-filter only decides whom to ask
 * the user about.
 */
private fun ctapInterface(device: UsbDevice): UsbInterface? =
    (0 until device.interfaceCount).map(device::getInterface).firstOrNull { i ->
        i.interfaceClass == UsbConstants.USB_CLASS_HID &&
            i.interfaceProtocol == 0 &&
            i.interruptEndpoint(UsbConstants.USB_DIR_IN) != null &&
            i.interruptEndpoint(UsbConstants.USB_DIR_OUT) != null
    }

private fun UsbInterface.interruptEndpoint(direction: Int): UsbEndpoint? =
    (0 until endpointCount).map(::getEndpoint).firstOrNull {
        it.type == UsbConstants.USB_ENDPOINT_XFER_INT && it.direction == direction && it.maxPacketSize == 64
    }

/** GET_DESCRIPTOR(Report) and look for `Usage Page (FIDO Alliance, 0xF1D0)`. */
private fun isFidoHid(conn: UsbDeviceConnection, iface: UsbInterface): Boolean {
    val buf = ByteArray(512)
    val n = conn.controlTransfer(
        UsbConstants.USB_DIR_IN or UsbConstants.USB_TYPE_STANDARD or 0x01, // interface recipient
        0x06, // GET_DESCRIPTOR
        0x2200, // HID report descriptor
        iface.id,
        buf,
        buf.size,
        1_000,
    )
    if (n <= 0) return true // some stacks refuse the request; the CTAPHID INIT below decides
    for (i in 0 until n - 2) {
        if (buf[i] == 0x06.toByte() && buf[i + 1] == 0xD0.toByte() && buf[i + 2] == 0xF1.toByte()) return true
    }
    return false
}

/** One CTAPHID pipe: 64-byte reports on the interrupt endpoints, no report id. */
private class UsbHidLink(private val conn: UsbDeviceConnection, private val iface: UsbInterface) : Fido2UsbLink, AutoCloseable {
    private val input = iface.interruptEndpoint(UsbConstants.USB_DIR_IN)!!
    private val output = iface.interruptEndpoint(UsbConstants.USB_DIR_OUT)!!

    @Volatile private var closed = false

    override fun writeReport(packet: ByteArray) {
        if (closed) throw gone()
        val n = conn.bulkTransfer(output, packet, packet.size, 1_000)
        if (n != packet.size) throw gone()
    }

    override fun readReport(timeoutMs: ULong): ByteArray? {
        if (closed) throw gone()
        val buf = ByteArray(64)
        val n = conn.bulkTransfer(input, buf, buf.size, timeoutMs.toLong().coerceIn(1, Int.MAX_VALUE.toLong()).toInt())
        return if (n <= 0) null else buf.copyOf(n)
    }

    override fun close() {
        closed = true
        runCatching { conn.releaseInterface(iface) }
        runCatching { conn.close() }
    }

    private fun gone() = MobileException.Other(kind = "io", detail = "USB security key disconnected")
}

/** One ISO-DEP tag: command APDU in, response APDU (with SW) out. */
private class NfcIsoDepLink(private val iso: IsoDep) : Fido2NfcLink, AutoCloseable {
    override fun transceive(apdu: ByteArray): ByteArray = try {
        iso.transceive(apdu)
    } catch (e: TagLostException) {
        throw MobileException.Other(kind = "io", detail = "Security key moved away from the NFC reader")
    } catch (e: IOException) {
        throw MobileException.Other(kind = "io", detail = e.message ?: "NFC transfer failed")
    }

    override fun extendedLength(): Boolean = iso.isExtendedLengthApduSupported

    override fun close() {
        runCatching { iso.close() }
    }
}

@Suppress("DEPRECATION")
private inline fun <reified T : android.os.Parcelable> Intent.getParcelableExtraCompat(name: String): T? =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) getParcelableExtra(name, T::class.java) else getParcelableExtra(name)
