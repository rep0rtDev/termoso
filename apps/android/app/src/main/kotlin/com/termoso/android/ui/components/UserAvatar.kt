package com.termoso.android.ui.components

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.termoso.android.data.VaultRepository
import com.termoso.android.ui.hosts.initial
import java.util.Collections

/**
 * Decoded profile pictures keyed by `user:tag`. The tag changes with the
 * picture, so an entry never goes stale; Rust keeps the bytes on disk.
 */
private object AvatarMemory {
    val hits = LruCache<String, ImageBitmap>(64)
    val misses: MutableSet<String> = Collections.synchronizedSet(HashSet())
}

/** The user's picture, or `null` while loading / when they have none. */
@Composable
fun rememberUserAvatar(repo: VaultRepository, userId: String?, tag: String?): ImageBitmap? {
    if (userId == null || tag.isNullOrEmpty()) return null
    val key = "$userId:$tag"
    val state by produceState(AvatarMemory.hits.get(key), key) {
        if (value != null || key in AvatarMemory.misses) return@produceState
        val bitmap = runCatching {
            repo.read { userAvatar(userId, tag) }?.let { bytes ->
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
            }
        }.getOrNull()
        if (bitmap == null) AvatarMemory.misses += key else AvatarMemory.hits.put(key, bitmap)
        value = bitmap
    }
    return state
}

/**
 * Square tile with the person's picture, falling back to their initial on a
 * solid tint until the picture is available.
 */
@Composable
fun UserAvatar(
    repo: VaultRepository,
    userId: String?,
    tag: String?,
    name: String,
    modifier: Modifier = Modifier,
    size: Int = 40,
    shape: Shape = RoundedCornerShape(10.dp),
    container: Color = MaterialTheme.colorScheme.primary,
    textStyle: TextStyle = MaterialTheme.typography.titleMedium,
) {
    val picture = rememberUserAvatar(repo, userId, tag)
    Box(
        modifier
            .size(size.dp)
            .clip(shape)
            .background(container),
        contentAlignment = Alignment.Center,
    ) {
        if (picture != null) {
            Image(picture, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        } else {
            Text(
                initial(name),
                color = Color.White,
                style = textStyle,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}
