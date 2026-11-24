package com.pandex.autofill

import android.app.assist.AssistStructure
import android.os.CancellationSignal
import android.service.autofill.AutofillService
import android.service.autofill.Dataset
import android.service.autofill.FillCallback
import android.service.autofill.FillRequest
import android.service.autofill.FillResponse
import android.service.autofill.SaveCallback
import android.service.autofill.SaveRequest
import android.util.Log
import android.view.View
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.widget.RemoteViews

class SpikeAutofillService : AutofillService() {
    @Suppress("DEPRECATION") // RemoteViews dataset presentation keeps API 26 support.
    override fun onFillRequest(request: FillRequest, signal: CancellationSignal, callback: FillCallback) {
        if (signal.isCanceled) return

        val structure = request.fillContexts.lastOrNull()?.structure
        val target = structure?.activityComponent
        if (structure == null || target?.packageName != packageName ||
            target.className != SpikeLoginActivity::class.java.name
        ) {
            callback.onSuccess(null)
            return
        }

        val username = structure.findField("pandex_spike_username", View.AUTOFILL_HINT_USERNAME)
        val password = structure.findField("pandex_spike_password", View.AUTOFILL_HINT_PASSWORD)
        if (username == null || password == null || username == password) {
            callback.onSuccess(null)
            return
        }

        val presentation = RemoteViews(packageName, android.R.layout.simple_list_item_1).apply {
            setTextViewText(android.R.id.text1, getText(R.string.pandex_spike_dataset))
        }
        val dataset = Dataset.Builder(presentation)
            .setValue(username, AutofillValue.forText(DEMO_USERNAME))
            .setValue(password, AutofillValue.forText(DEMO_PASSWORD))
            .build()
        if (signal.isCanceled) return
        callback.onSuccess(FillResponse.Builder().addDataset(dataset).build())
        Log.i("PandexM0Autofill", "Returned one demo dataset for the test login")
    }

    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        callback.onSuccess()
    }

    private fun AssistStructure.findField(entry: String, hint: String): AutofillId? {
        for (index in 0 until windowNodeCount) {
            getWindowNodeAt(index).rootViewNode.findField(entry, hint)?.let { return it }
        }
        return null
    }

    private fun AssistStructure.ViewNode.findField(entry: String, hint: String): AutofillId? {
        if (idEntry == entry && autofillType == View.AUTOFILL_TYPE_TEXT &&
            autofillHints?.contains(hint) == true
        ) {
            return autofillId
        }
        for (index in 0 until childCount) {
            getChildAt(index).findField(entry, hint)?.let { return it }
        }
        return null
    }
}
