package com.termoso.android.data

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.net.ConnectivityManager
import android.os.Environment
import com.termoso.core.LinuxDistro
import com.termoso.core.LinuxInstallListener
import com.termoso.core.LinuxInstalled
import com.termoso.core.LinuxShell
import com.termoso.core.LinuxStage
import com.termoso.core.linuxDistros
import com.termoso.core.linuxInstall
import com.termoso.core.linuxInstallFile
import com.termoso.core.linuxInstallUrl
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

    /** The installed system's root directory, what its shells see as `/`; null when none is installed. */
    val filesRoot: File? get() = if (isInstalled) File(rootDir, ROOTFS) else null

    /** `Ubuntu 24.04 LTS` and the like, for labels. */
    val title: String? get() = _installed.value?.let { "${it.name} ${it.version}" }

    fun refresh() {
        _installed.value = linuxInstalled(rootDir.absolutePath)
    }

    /** Download and unpack `distroId`; progress and the result arrive on the flows. */
    fun install(distroId: String) {
        start(distroId) { linuxInstall(rootDir.absolutePath, distroId, dnsServers(), it) }
    }

    /**
     * Copy a user-picked archive (SAF) into our cache and install it as a
     * custom root filesystem. The copy is deleted by Rust after unpacking.
     */
    suspend fun installFromUri(uri: Uri) {
        if (_progress.value != null) return
        _error.value = null
        val resolver = context.contentResolver
        val display = runCatching {
            resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        }.getOrNull() ?: uri.lastPathSegment ?: "rootfs"
        val name = display.removeSuffix(".xz").removeSuffix(".gz").removeSuffix(".tar").removeSuffix(".tgz").removeSuffix(".txz")
        val copy = File(context.cacheDir, "rootfs-import.img")
        val copied = withContext(Dispatchers.IO) {
            runCatching {
                resolver.openInputStream(uri)!!.use { input -> copy.outputStream().use { input.copyTo(it) } }
            }
        }
        if (copied.isFailure) {
            copy.delete()
            _error.value = copied.exceptionOrNull()?.message ?: "Could not read the file"
            return
        }
        start(name) { linuxInstallFile(rootDir.absolutePath, copy.absolutePath, name, dnsServers(), it) }
    }

    /** Download an archive from `url` (optionally verified against `sha256`) and install it. */
    fun installFromUrl(url: String, sha256: String?, name: String) {
        val label = name.trim().ifEmpty { url.trim().substringAfterLast('/').ifEmpty { "Custom Linux" } }
        start(label) {
            linuxInstallUrl(rootDir.absolutePath, url.trim(), sha256?.trim()?.ifEmpty { null }, label, dnsServers(), it)
        }
    }

    /** `label` is what the progress card shows: a distro id or a custom image's name. */
    private fun start(label: String, run: (LinuxInstallListener) -> Unit) {
        if (_progress.value != null) return
        _error.value = null
        _progress.value = LinuxInstallProgress(label, LinuxStage.DOWNLOADING, 0u, 0u)
        run(
            object : LinuxInstallListener {
                override fun onProgress(stage: LinuxStage, done: ULong, total: ULong) {
                    _progress.value = LinuxInstallProgress(label, stage, done, total)
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

    private companion object {
        /** Mirrors `ROOTFS` in `termoso-mobile/src/linux.rs`. */
        const val ROOTFS = "rootfs"
    }
}
