package com.omnisearch.app.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import com.omnisearch.app.MainActivity
import com.omnisearch.app.R

class ScanConnectWidgetProvider : AppWidgetProvider() {

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        for (appWidgetId in appWidgetIds) {
            updateAppWidget(context, appWidgetManager, appWidgetId)
        }
    }

    companion object {
        fun updateAppWidget(context: Context, appWidgetManager: AppWidgetManager, appWidgetId: Int) {
            val views = RemoteViews(context.packageName, R.layout.widget_scan_connect)
            
            // Intent to open MainActivity with ACTION_OPEN_SCANNER
            val intent = Intent(context, MainActivity::class.java).apply {
                action = "com.omnisearch.app.ACTION_OPEN_SCANNER"
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            }
            
            val pendingIntent = PendingIntent.getActivity(
                context,
                0, // Request code
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
            
            // Make the entire widget clickable
            views.setOnClickPendingIntent(R.id.widget_root, pendingIntent)
            
            WidgetThemeUtil.applyThemeLayoutBg(context, views, R.id.widget_scan_bg,
                listOf(R.id.widget_text_view),
                emptyList()
            )
            
            appWidgetManager.updateAppWidget(appWidgetId, views)
        }
    }
}
