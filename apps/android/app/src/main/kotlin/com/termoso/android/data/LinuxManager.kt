package com.termoso.android.data

import android.content.Context
import android.net.ConnectivityManager
import android.os.Environment
import com.termoso.core.LinuxDistro
import com.termoso.core.LinuxInstallListener
import com.termoso.core.LinuxInstalled
import com.termoso.core.LinuxShell
import com.termoso.core.LinuxStage
import com.termoso.core.linuxDistros
import com.termoso.core.linuxInstall
import com.termoso.core.linuxInstalled
import com.termoso.core.linuxRemove
import com.termoso.core.linuxShell
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext

/** Progress of a running install, for the screen. */
data class LinuxInstallProgress(val distroId: String, val stage: LinuxStage, val done: ULong, val total: ULong)

/**
 * The Linux userland for the local shell: an official distro rootfs under
 * `files/linux/rootfs`, run by the bundled proot. All work is in Rust
 * (`linux.rs`); this keeps the state the UI observes and knows the
 * Android-side paths (native library dir, DNS servers, external storage).
 */
class LinuxManager(private val context: Context) {
    private val rootDir: File = File(context.filesDir, "linux")
    private val libDir: String get() = context.applicationInfo.nativeLibraryDir

    private val _installed = MutableStateFlow(linuxInstalled(rootDir.absolutePath))
    val installed: StateFlow<LinuxInstalled?> = _installed.asStateFlow()

    private val _progress = MutableStateFlow<LinuxInstallProgress?>(null)
    val progress: StateFlow<LinuxInstallProgress?> = _progress.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    val distros: List<LinuxDistro> = linuxDistros()

    val isInstalled: Boolean get() = _installed.value != null

    fun refresh() {
        _installed.value = linuxInstalled(rootDir.absolutePath)
    }

    /** Download and unpack `distroId`; progress and the result arrive on the flows. */
    fun install(distroId: String) {
        if (_progress.value != null) return
        _error.value = null
        _progress.value = LinuxInstallProgress(distroId, LinuxStage.DOWNLOADING, 0u, 0u)
        linuxInstall(
            rootDir.absolutePath,
            distroId,
            dnsServers(),
            object : LinuxInstallListener {
                override fun onProgress(stage: LinuxStage, done: ULong, total: ULong) {
                    _progress.value = LinuxInstallProgress(distroId, stage, done, total)
                }

                override fun onFinished(error: String?) {
                    _progress.value = null
                    _error.value = error
                    refresh()
                }
            },
        )
    }

    fun clearError() {
        _error.value = null
    }

    suspend fun remove() {
        withContext(Dispatchers.IO) { linuxRemove(rootDir.absolutePath) }
        refresh()
    }

    /** Command line for a shell inside the installed system, or null when there is none. */
    fun shell(): LinuxShell? {
        if (!isInstalled) return null
        val shared = Environment.getExternalStorageDirectory()?.takeIf { it.canRead() }?.absolutePath
        return runCatching { linuxShell(rootDir.absolutePath, libDir, dnsServers(), shared) }.getOrNull()
    }

    private fun dnsServers(): List<String> {
        val cm = context.getSystemService(ConnectivityManager::class.java) ?: return emptyList()
        val props = cm.activeNetwork?.let(cm::getLinkProperties) ?: return emptyList()
        return props.dnsServers.mapNotNull { it.hostAddress }
    }
}
