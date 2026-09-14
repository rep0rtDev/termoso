package com.termoso.android.ui.sftp

import com.termoso.core.TransferCard
import com.termoso.core.TransferStatus

/** Buttons a transfer card offers, in display order. */
enum class TransferAction { Pause, Resume, Retry, Cancel, Dismiss }

/** Done, failed or cancelled: the Rust queue is finished with it. */
val TransferStatus.isFinished: Boolean
    get() = this is TransferStatus.Done || this is TransferStatus.Failed || this is TransferStatus.Cancelled

/** Pause/Cancel while owned by the queue, Resume/Retry for anything with a partial file, Dismiss once final. */
val TransferStatus.actions: List<TransferAction>
    get() = when (this) {
        is TransferStatus.Queued, is TransferStatus.Running -> listOf(TransferAction.Pause, TransferAction.Cancel)
        is TransferStatus.Paused -> listOf(TransferAction.Resume, TransferAction.Cancel)
        is TransferStatus.Failed -> listOf(TransferAction.Retry, TransferAction.Dismiss)
        is TransferStatus.Done, is TransferStatus.Cancelled -> listOf(TransferAction.Dismiss)
    }

/** Whether the row shows a progress bar (determinate when the total is known). */
val TransferStatus.showsProgress: Boolean
    get() = this is TransferStatus.Queued || this is TransferStatus.Running || this is TransferStatus.Paused

/** One-line status under the path: bytes so far, speed while moving, the error when failed. */
fun TransferCard.statusLabel(): String {
    val total = total
    val progress = buildString {
        append(formatSize(done))
        if (total != null) append(" / ").append(formatSize(total))
    }
    return when (val s = status) {
        is TransferStatus.Queued -> if (done > 0uL) "Queued · $progress" else "Queued"
        is TransferStatus.Running -> buildString {
            append(progress)
            if (bytesPerSec > 0uL) append(" · ").append(formatSize(bytesPerSec)).append("/s")
        }
        is TransferStatus.Paused -> "Paused · $progress"
        is TransferStatus.Done -> "Done · ${formatSize(done)}"
        is TransferStatus.Failed -> if (done > 0uL) "${s.message} · $progress" else s.message
        is TransferStatus.Cancelled -> "Cancelled"
    }
}
