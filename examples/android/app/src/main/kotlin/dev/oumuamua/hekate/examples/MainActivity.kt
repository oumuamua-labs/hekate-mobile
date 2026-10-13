// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

package dev.oumuamua.hekate.examples

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.RadioGroup
import android.widget.Spinner
import android.widget.TextView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uniffi.example_prover.proverBuild

class MainActivity : Activity() {
    private val scope = MainScope()

    private var statement = Statement.RSA
    private var sizes: List<InputSize> = emptyList()
    private var inputSize: InputSize? = null
    private var job: Job? = null

    private lateinit var statements: RadioGroup
    private lateinit var inputSizes: Spinner
    private lateinit var claim: TextView
    private lateinit var run: Button
    private lateinit var cancel: Button
    private lateinit var status: TextView
    private lateinit var report: TextView
    private lateinit var share: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.main)

        statements = findViewById(R.id.statement)
        inputSizes = findViewById(R.id.input_size)
        claim = findViewById(R.id.claim)
        run = findViewById(R.id.run)
        cancel = findViewById(R.id.cancel)
        status = findViewById(R.id.status)
        report = findViewById(R.id.report)
        share = findViewById(R.id.share)

        statements.setOnCheckedChangeListener { _, checked ->
            statement = when (checked) {
                R.id.statement_mldsa -> Statement.ML_DSA
                R.id.statement_keccak -> Statement.KECCAK
                R.id.statement_sha256 -> Statement.SHA256
                else -> Statement.RSA
            }

            showStatement()
        }

        inputSizes.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) {
                inputSize = sizes.getOrNull(position)
            }

            override fun onNothingSelected(parent: AdapterView<*>?) {
                inputSize = null
            }
        }

        run.setOnClickListener { start() }
        cancel.setOnClickListener { job?.cancel() }
        share.setOnClickListener { shareReport() }

        showStatement()
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun showStatement() {
        val reason = statement.unavailableReason()

        sizes = statement.inputSizes()
        inputSize = sizes.firstOrNull()

        inputSizes.adapter = ArrayAdapter(
            this,
            android.R.layout.simple_spinner_dropdown_item,
            sizes.map(InputSize::label),
        )
        inputSizes.visibility = if (sizes.isEmpty()) View.GONE else View.VISIBLE

        claim.text = "${statement.claim}\n\nPublic: ${statement.publicValues}\nHidden: ${statement.hiddenValues}"
        run.isEnabled = reason == null
        status.text = reason.orEmpty()
        report.text = ""
        share.visibility = View.GONE
    }

    private fun start() {
        val statement = statement
        val inputSize = inputSize

        showRunning(true)
        status.text = "Preparing"
        report.text = ""
        share.visibility = View.GONE

        job = scope.launch {
            try {
                val result = measure(statement, inputSize)

                status.text = result.verdict
                report.text = result.text
                share.visibility = View.VISIBLE
            } catch (e: CancellationException) {
                status.text = "Cancelled"

                throw e
            } catch (e: Exception) {
                status.text = e.message ?: e.toString()
            } catch (e: LinkageError) {
                status.text = "a native library did not load: ${e.message}"
            } finally {
                showRunning(false)
            }
        }
    }

    private suspend fun measure(statement: Statement, inputSize: InputSize?): Report {
        val before = DeviceConditions.current(this)
        val memoryBefore = Memory.read()

        val measurement = Benchmark.measure(statement, inputSize) { run ->
            status.text = "Proof $run of ${Benchmark.RUNS}"
        }

        return Report(
            statement = statement,
            inputSize = inputSize,
            before = before,
            thermalStatusAfter = DeviceConditions.currentThermalStatus(this),
            hekateVersion = BuildConfig.HEKATE_VERSION,
            prover = proverBuild(),
            measurement = measurement,
            memoryBefore = memoryBefore,
            memoryAfter = Memory.read(),
        )
    }

    private fun showRunning(running: Boolean) {
        run.visibility = if (running) View.GONE else View.VISIBLE
        cancel.visibility = if (running) View.VISIBLE else View.GONE
        inputSizes.isEnabled = !running

        for (index in 0 until statements.childCount) {
            statements.getChildAt(index).isEnabled = !running
        }
    }

    private fun shareReport() {
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_TEXT, "${status.text}\n${report.text}")

        startActivity(Intent.createChooser(send, null))
    }
}