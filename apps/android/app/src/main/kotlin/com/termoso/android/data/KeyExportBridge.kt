package com.termoso.android.data

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import com.termoso.core.KeyExportListener
import com.termoso.core.KeyExportResult
import com.termoso.core.PromptRequest

/** Outcome of one ssh-copy-id run. */
sealed interface KeyExportState {
    data class Running(val detail: String) : KeyExportState
    data class Done(val result: KeyExportResult) : KeyExportState
    data class Failed(val message: String) : KeyExportState
}

/** Rust `KeyExport` callbacks republished as flows for the UI. */
class KeyExportBridge : KeyExportListener {
    private val _state = MutableStateFlow<KeyExportState>(KeyExportState.Running(""))
    val state: StateFlow<KeyExportState> = _state.asStateFlow()
    private val _prompt = MutableStateFlow<PendingPrompt?>(null)
    val prompt: StateFlow<PendingPrompt?> = _prompt.asStateFlow()

    override fun onStage(detail: String) {
        _state.value = KeyExportState.Running(detail)
    }

    override fun onPrompt(promptId: ULong, request: PromptRequest) {
        _prompt.value = PendingPrompt(promptId, request)
    }

    override fun onDone(result: KeyExportResult) {
        _prompt.value = null
        _state.value = KeyExportState.Done(result)
    }

    override fun onFailed(message: String) {
        _prompt.value = null
        _state.value = KeyExportState.Failed(message)
    }

    fun promptAnswered(id: ULong) {
        _prompt.update { if (it?.id == id) null else it }
    }
}
