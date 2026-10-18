package com.termoso.android.ui.account

import android.annotation.SuppressLint
import android.graphics.Color
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.net.toUri
import com.termoso.android.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch

/**
 * "Confirm you are human": the server's `/captcha` page (Cloudflare
 * Turnstile) in a bottom sheet. The page hands the token to the
 * `TermosoCaptcha` JavaScript bridge; nothing else runs in this web view and
 * navigation is pinned to the server and Cloudflare.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CaptchaSheet(serverUrl: String, onToken: (String) -> Unit, onDismiss: () -> Unit) {
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val origin = serverUrl.toUri().let { "${it.scheme}://${it.authority}" }
    val pageUrl = "$origin/captcha?theme=${if (dark) "dark" else "light"}"
    val deliver by rememberUpdatedState(onToken)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 32.dp)) {
            Text(stringResource(R.string.confirm_you_are_human), style = MaterialTheme.typography.titleMedium)
            Text(
                stringResource(R.string.captcha_sheet_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 12.dp),
            )
            CaptchaWebView(pageUrl, origin, onToken = { deliver(it) }, modifier = Modifier.fillMaxWidth().height(96.dp))
        }
    }
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun CaptchaWebView(pageUrl: String, origin: String, onToken: (String) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    AndroidView(
        modifier = modifier,
        factory = {
            WebView(context).apply {
                setBackgroundColor(Color.TRANSPARENT)
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                settings.setSupportMultipleWindows(false)
                webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                        val host = request.url.host.orEmpty()
                        val ours = request.url.toString().startsWith(origin)
                        return !(ours || host == "challenges.cloudflare.com" || host.endsWith(".cloudflare.com"))
                    }
                }
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun onToken(token: String?) {
                            if (token.isNullOrBlank()) return
                            MainScope().launch(Dispatchers.Main) { onToken(token) }
                        }
                    },
                    "TermosoCaptcha",
                )
                loadUrl(pageUrl)
            }
        },
        onRelease = { it.destroy() },
    )
}
