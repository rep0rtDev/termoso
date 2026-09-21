package com.termoso.android.ui.components

import com.termoso.android.R
import com.termoso.android.str
import com.termoso.core.ConnectStage
import com.termoso.core.SessionState
import com.termoso.core.TunnelState

/** Localised text for a connection stage; a jump host is prefixed as `hop: …`. */
fun connectingLabel(stage: ConnectStage, hop: String?): String {
    val base = when (stage) {
        is ConnectStage.Connecting -> str(R.string.connecting_ellipsis)
        is ConnectStage.Resolving -> str(R.string.stage_resolving_host)
        is ConnectStage.ConnectingTo -> str(R.string.stage_connecting_to, stage.target)
        is ConnectStage.Handshake -> str(R.string.stage_handshake)
        is ConnectStage.HostKey -> str(R.string.stage_checking_host_key)
        is ConnectStage.Auth -> str(R.string.stage_authenticating, stage.method)
        is ConnectStage.SecurityKeyTouch -> str(R.string.stage_touch_your_security_key)
        is ConnectStage.Authenticated -> str(R.string.stage_authenticated)
        is ConnectStage.MoshServer -> str(R.string.stage_starting_mosh_server)
        is ConnectStage.MoshWaiting -> str(R.string.stage_mosh_waiting, stage.port.toInt())
        is ConnectStage.OpeningShell -> str(R.string.stage_opening_shell)
        is ConnectStage.StartingShell -> str(R.string.stage_starting_shell)
        is ConnectStage.OpeningSftp -> str(R.string.stage_opening_sftp)
        is ConnectStage.OpeningTunnel -> str(R.string.stage_opening_tunnel)
    }
    return if (hop != null) str(R.string.stage_via_hop, hop, base) else base
}

fun connectingLabel(state: SessionState.Connecting): String = connectingLabel(state.stage, state.hop)

fun connectingLabel(state: TunnelState.Connecting): String = connectingLabel(state.stage, state.hop)

/** The placeholder state before Rust reports its first stage. */
fun initialConnecting(): SessionState.Connecting =
    SessionState.Connecting(str(R.string.connecting_ellipsis), ConnectStage.Connecting, null)

fun initialTunnelConnecting(): TunnelState.Connecting =
    TunnelState.Connecting(str(R.string.connecting_ellipsis), ConnectStage.Connecting, null)
