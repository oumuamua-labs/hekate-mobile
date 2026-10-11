// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples.signatures

import android.content.Intent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SignatureExampleTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val device = UiDevice.getInstance(instrumentation)
    private val app = instrumentation.targetContext.packageName

    @Before
    fun launch() {
        val context = instrumentation.targetContext

        val intent = context.packageManager.getLaunchIntentForPackage(app)
            ?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)

        assertNotNull("the app has no launch intent", intent)
        context.startActivity(intent)

        view("run")
    }

    @Test
    fun rsaSignatureMadeOnDeviceProvesAndVerifies() {
        runAndExpectThreeVerifiedProofs(reportName = "rsa")
    }

    @Test
    fun mlDsaSignatureMadeOnDeviceProvesAndVerifies() {
        view("statement_mldsa").click()

        val run = view("run")

        assumeTrue(view("status").text.orEmpty(), run.isEnabled)

        runAndExpectThreeVerifiedProofs(reportName = "ml-dsa")
    }

    @Test
    fun cancelStopsRunningProof() {
        view("run").click()
        view("cancel").click()

        assertNotNull(
            "the run did not report Cancelled",
            device.wait(Until.findObject(By.res(app, "status").text("Cancelled")), 30_000),
        )
    }

    private fun runAndExpectThreeVerifiedProofs(reportName: String) {
        view("run").click()

        val share = device.wait(Until.findObject(By.res(app, "share")), 180_000)
        val status = view("status").text

        assertNotNull("no report within 180 seconds, status: $status", share)
        assertEquals("3 of 3 proofs verified", status)

        keep(view("report").text, reportName)
    }

    private fun keep(report: String, name: String) {
        val directory = InstrumentationRegistry.getArguments().getString("additionalTestOutputDir")
            ?: return

        File(directory, "$name.txt").writeText(report)
    }

    private fun view(id: String): UiObject2 {
        val found = device.wait(Until.findObject(By.res(app, id)), 10_000)

        assertNotNull("no view $id", found)

        return found
    }
}