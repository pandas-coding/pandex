package com.pandex.autofill

import android.app.Activity
import android.os.Bundle
import android.text.InputType
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

internal const val DEMO_USERNAME = "pandex-m0@example.invalid"
internal const val DEMO_PASSWORD = "m0-demo-not-a-secret"

class SpikeLoginActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val username = EditText(this).apply {
            id = R.id.pandex_spike_username
            setHint(R.string.pandex_spike_username)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
            setAutofillHints(View.AUTOFILL_HINT_USERNAME)
        }
        val password = EditText(this).apply {
            id = R.id.pandex_spike_password
            setHint(R.string.pandex_spike_password)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            setAutofillHints(View.AUTOFILL_HINT_PASSWORD)
        }
        val result = TextView(this)
        val check = Button(this).apply {
            setText(R.string.pandex_spike_check)
            setOnClickListener {
                val matches = DEMO_USERNAME.contentEquals(username.text) &&
                    DEMO_PASSWORD.contentEquals(password.text)
                result.setText(if (matches) R.string.pandex_spike_match else R.string.pandex_spike_mismatch)
            }
        }
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            isFocusableInTouchMode = true
            val padding = (24 * resources.displayMetrics.density).toInt()
            setPadding(padding, padding, padding, padding)
            addView(TextView(context).apply { setText(R.string.pandex_spike_login) })
            addView(username)
            addView(password)
            addView(check)
            addView(result)
            requestFocus()
        })
    }
}
