package com.termoso.android

import android.app.Application
import org.junit.Before
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/** Base for JVM tests of presentation helpers that resolve string resources through [str]. */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [35])
abstract class ResourceTest {
    @Before
    fun initStrings() {
        Strings.init(RuntimeEnvironment.getApplication())
    }
}
