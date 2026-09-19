package com.termoso.android.ui.keychain

import com.termoso.android.str
import com.termoso.android.R
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.PersistableBundle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Copy text; `sensitive` hides it from the Android 13+ clipboard preview overlay. */
fun copyText(context: Context, label: String, text: String, sensitive: Boolean = false) {
    val clip = ClipData.newPlainText(label, text)
    if (sensitive) {
        clip.description.extras = PersistableBundle().apply {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
            } else {
                putBoolean("android.content.extra.IS_SENSITIVE", true)
            }
        }
    }
    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(clip)
}

fun pasteText(context: Context): String? =
    context.getSystemService(ClipboardManager::class.java)
        .primaryClip?.takeIf { it.itemCount > 0 }
        ?.getItemAt(0)?.coerceToText(context)?.toString()
        ?.takeIf { it.isNotBlank() }

private const val MAX_KEY_FILE_BYTES = 256 * 1024

/** Read a picked document as UTF-8 text, refusing anything too large to be a key or certificate. */
suspend fun readTextFile(context: Context, uri: Uri): String = withContext(Dispatchers.IO) {
    val bytes = context.contentResolver.openInputStream(uri)?.use { input ->
        val buf = input.readNBytes(MAX_KEY_FILE_BYTES + 1)
        require(buf.size <= MAX_KEY_FILE_BYTES) { str(R.string.file_is_too_large_to_be_a_key) }
        buf
    } ?: error(str(R.string.could_not_open_the_file))
    bytes.toString(Charsets.UTF_8)
}
