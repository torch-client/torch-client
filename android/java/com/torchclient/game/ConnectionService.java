package com.torchclient.game;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.IBinder;

public final class ConnectionService extends Service {
    private static final String CHANNEL = "connection";
    private static final String EXTRA_ADDRESS = "address";
    private static final int ID = 1;
    private static ConnectionService running;
    private static boolean stopPending;
    private static String pendingReason;

    public static void start(final Activity activity, final String address) {
        activity.runOnUiThread(() -> {
            stopPending = false;
            pendingReason = null;
            if (Build.VERSION.SDK_INT >= 33
                    && activity.checkSelfPermission("android.permission.POST_NOTIFICATIONS")
                        != PackageManager.PERMISSION_GRANTED) {
                activity.requestPermissions(new String[] {"android.permission.POST_NOTIFICATIONS"}, 1);
            }
            try {
                Intent i = new Intent(activity, ConnectionService.class);
                i.putExtra(EXTRA_ADDRESS, address);
                activity.startForegroundService(i);
            } catch (RuntimeException e) {
            }
        });
    }

    public static void stop(final Activity activity, final String reason) {
        activity.runOnUiThread(() -> {
            if (running != null) {
                running.finish(reason);
            } else {
                stopPending = true;
                pendingReason = reason;
            }
        });
    }

    public static void clear(final Activity activity) {
        activity.runOnUiThread(() -> {
            if (running == null) {
                activity.getSystemService(NotificationManager.class).cancel(ID);
            }
        });
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        getSystemService(NotificationManager.class).createNotificationChannel(
                new NotificationChannel(CHANNEL, "Server connection", NotificationManager.IMPORTANCE_LOW));
        String address = intent == null ? null : intent.getStringExtra(EXTRA_ADDRESS);
        startForeground(ID, builder("Connected to server", address == null ? "" : address)
                .setSmallIcon(android.R.drawable.presence_online)
                .setUsesChronometer(true)
                .build());
        running = this;
        if (stopPending) {
            stopPending = false;
            String reason = pendingReason;
            pendingReason = null;
            finish(reason);
        }
        return START_NOT_STICKY;
    }

    private void finish(String reason) {
        running = null;
        if (reason == null) {
            stopForeground(STOP_FOREGROUND_REMOVE);
        } else {
            stopForeground(STOP_FOREGROUND_DETACH);
            getSystemService(NotificationManager.class).notify(ID, builder("Disconnected from server", reason)
                    .setSmallIcon(android.R.drawable.presence_offline)
                    .setStyle(new Notification.BigTextStyle().bigText(reason))
                    .setAutoCancel(true)
                    .build());
        }
        stopSelf();
    }

    private Notification.Builder builder(String title, String text) {
        Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        return new Notification.Builder(this, CHANNEL)
                .setContentTitle(title)
                .setContentText(text)
                .setContentIntent(PendingIntent.getActivity(this, 0, launch, PendingIntent.FLAG_IMMUTABLE));
    }

    @Override
    public void onDestroy() {
        if (running == this) {
            running = null;
        }
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
