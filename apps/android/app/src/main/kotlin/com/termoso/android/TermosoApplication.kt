package com.termoso.android

import android.app.Application
import com.termoso.android.data.AppContainer
import com.termoso.core.initLogging

class TermosoApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        initLogging(verbose = BuildConfig.DEBUG)
        container = AppContainer(this)
    }
}
