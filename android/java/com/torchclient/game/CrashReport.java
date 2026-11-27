package com.torchclient.game;

import android.app.Activity;
import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.graphics.Color;
import android.graphics.Typeface;
import android.os.Build;
import android.os.Bundle;
import android.os.Process;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;

public final class CrashReport extends Activity {
    private static final String PREFS = "crash_report";
    private static final String SEEN_TIME = "seen_time";
    private static final String SEEN_LINE = "seen_line";
    private static final String SEEN_PANIC = "seen_panic";
    private static final int LOG_LINES = 1000;
    private static final int TRACE_CHARS = 100_000;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        String report;
        try {
            report = findCrash();
        } catch (Throwable t) {
            report = null;
        }
        if (report == null) {
            startGame();
            return;
        }
        show(report);
    }

    private void startGame() {
        Intent game = new Intent(this, android.app.NativeActivity.class);
        Bundle extras = getIntent().getExtras();
        if (extras != null) {
            game.putExtras(extras);
        }
        startActivity(game);
        overridePendingTransition(0, 0);
        finish();
    }

    private String findCrash() {
        SharedPreferences prefs = getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        SharedPreferences.Editor seen = prefs.edit();
        StringBuilder found = new StringBuilder();
        List<String> log = null;

        File panic = newestPanic();
        if (panic != null && panic.lastModified() > prefs.getLong(SEEN_PANIC, 0)) {
            seen.putLong(SEEN_PANIC, panic.lastModified());
            found.append("\n--- ").append(panic.getName()).append(" ---\n").append(readFile(panic));
        }

        if (Build.VERSION.SDK_INT >= 30) {
            ApplicationExitInfo exit = lastExit();
            if (exit != null && exit.getTimestamp() > prefs.getLong(SEEN_TIME, 0) && isCrash(exit.getReason())) {
                seen.putLong(SEEN_TIME, exit.getTimestamp());
                found.insert(0, exitText(exit));
            }
        } else {
            log = ownLog();
            String last = null;
            String me = " " + Process.myPid() + " ";
            for (String line : log) {
                if (!line.contains(me) && (line.contains("Fatal signal") || line.contains("= PANIC =") || line.contains("FATAL EXCEPTION"))) {
                    last = line;
                }
            }
            if (last != null && !last.equals(prefs.getString(SEEN_LINE, null))) {
                seen.putString(SEEN_LINE, last);
                found.insert(0, "Exit: " + last.trim() + "\n");
            }
        }

        if (found.length() == 0) {
            return null;
        }
        seen.apply();
        StringBuilder out = header();
        out.append(found);
        appendLog(out, log != null ? log : ownLog());
        return out.toString();
    }

    private static String exitText(ApplicationExitInfo exit) {
        StringBuilder out = new StringBuilder();
        out.append("Exit: ").append(reasonName(exit.getReason()));
        if (exit.getStatus() != 0) {
            out.append(" (status ").append(exit.getStatus()).append(')');
        }
        out.append('\n');
        out.append("Time: ").append(new SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.ROOT).format(new Date(exit.getTimestamp()))).append('\n');
        if (exit.getDescription() != null) {
            out.append("Description: ").append(exit.getDescription()).append('\n');
        }
        if (exit.getReason() == ApplicationExitInfo.REASON_ANR) {
            String trace = trace(exit);
            if (trace != null) {
                out.append("\n--- ANR trace ---\n").append(trace).append('\n');
            }
        }
        return out.toString();
    }

    private File newestPanic() {
        File[] files = new File(getFilesDir(), "logs").listFiles((dir, name) -> name.endsWith("-crash-log.txt"));
        File newest = null;
        if (files != null) {
            for (File f : files) {
                if (newest == null || f.lastModified() > newest.lastModified()) {
                    newest = f;
                }
            }
        }
        return newest;
    }

    private static String readFile(File f) {
        try (BufferedReader r = new BufferedReader(new InputStreamReader(new FileInputStream(f), StandardCharsets.UTF_8))) {
            StringBuilder out = new StringBuilder();
            String line;
            while ((line = r.readLine()) != null && out.length() < TRACE_CHARS) {
                out.append(line).append('\n');
            }
            return out.toString();
        } catch (Throwable t) {
            return "could not be read: " + t + "\n";
        }
    }

    private ApplicationExitInfo lastExit() {
        ActivityManager am = (ActivityManager) getSystemService(Context.ACTIVITY_SERVICE);
        List<ApplicationExitInfo> exits = am.getHistoricalProcessExitReasons(getPackageName(), 0, 8);
        for (ApplicationExitInfo e : exits) {
            if (getPackageName().equals(e.getProcessName())) {
                return e;
            }
        }
        return null;
    }

    private static boolean isCrash(int reason) {
        return reason == ApplicationExitInfo.REASON_CRASH
            || reason == ApplicationExitInfo.REASON_CRASH_NATIVE
            || reason == ApplicationExitInfo.REASON_ANR
            || reason == ApplicationExitInfo.REASON_INITIALIZATION_FAILURE;
    }

    private static String reasonName(int reason) {
        switch (reason) {
            case ApplicationExitInfo.REASON_CRASH:
                return "Java crash";
            case ApplicationExitInfo.REASON_CRASH_NATIVE:
                return "native crash";
            case ApplicationExitInfo.REASON_ANR:
                return "not responding (ANR)";
            case ApplicationExitInfo.REASON_INITIALIZATION_FAILURE:
                return "failed to start";
            default:
                return "reason " + reason;
        }
    }

    private static String trace(ApplicationExitInfo exit) {
        try (InputStream in = exit.getTraceInputStream()) {
            if (in == null) {
                return null;
            }
            BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8));
            StringBuilder out = new StringBuilder();
            String line;
            while ((line = r.readLine()) != null && out.length() < TRACE_CHARS) {
                out.append(line).append('\n');
            }
            return out.toString();
        } catch (Throwable t) {
            return null;
        }
    }

    private static List<String> ownLog() {
        List<String> lines = new ArrayList<>();
        try {
            java.lang.Process p = Runtime.getRuntime().exec(new String[] {"logcat", "-d", "-v", "threadtime", "-t", String.valueOf(LOG_LINES)});
            try (BufferedReader r = new BufferedReader(new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = r.readLine()) != null) {
                    lines.add(line);
                }
            }
            p.waitFor();
        } catch (Throwable t) {
            lines.add("logcat could not be read: " + t);
        }
        return lines;
    }

    private StringBuilder header() {
        StringBuilder out = new StringBuilder();
        String version = "?";
        try {
            version = getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Throwable ignored) {
        }
        out.append("Torch Client ").append(version).append('\n');
        out.append("Device: ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append('\n');
        out.append("Android: ").append(Build.VERSION.RELEASE).append(" (API ").append(Build.VERSION.SDK_INT).append(")\n");
        out.append("ABI: ").append(Build.SUPPORTED_ABIS.length > 0 ? Build.SUPPORTED_ABIS[0] : "?").append('\n');
        return out;
    }

    private static void appendLog(StringBuilder out, List<String> log) {
        out.append("\n--- log ---\n");
        for (String line : log) {
            out.append(line).append('\n');
        }
    }

    private void show(String report) {
        int pad = (int) (12 * getResources().getDisplayMetrics().density);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Color.rgb(24, 24, 24));
        root.setPadding(pad, pad, pad, pad);

        TextView title = new TextView(this);
        title.setText("Torch Client hit an error last time");
        title.setTextColor(Color.WHITE);
        title.setTextSize(20);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        root.addView(title);

        TextView hint = new TextView(this);
        hint.setText("Copy or share this report when you report the bug.");
        hint.setTextColor(Color.LTGRAY);
        hint.setPadding(0, pad / 2, 0, pad / 2);
        root.addView(hint);

        TextView body = new TextView(this);
        body.setText(report);
        body.setTextColor(Color.rgb(220, 220, 220));
        body.setTypeface(Typeface.MONOSPACE);
        body.setTextSize(11);
        body.setTextIsSelectable(true);
        HorizontalScrollView wide = new HorizontalScrollView(this);
        wide.addView(body);
        ScrollView tall = new ScrollView(this);
        tall.setBackgroundColor(Color.BLACK);
        tall.addView(wide);
        root.addView(tall, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));

        LinearLayout buttons = new LinearLayout(this);
        buttons.setOrientation(LinearLayout.HORIZONTAL);
        buttons.setGravity(Gravity.END);
        buttons.setPadding(0, pad / 2, 0, 0);
        buttons.addView(button("Copy", v -> {
            ClipboardManager cm = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
            cm.setPrimaryClip(ClipData.newPlainText("Torch Client crash report", report));
            Toast.makeText(this, "Copied", Toast.LENGTH_SHORT).show();
        }));
        buttons.addView(button("Share", v -> {
            Intent send = new Intent(Intent.ACTION_SEND);
            send.setType("text/plain");
            send.putExtra(Intent.EXTRA_SUBJECT, "Torch Client crash report");
            send.putExtra(Intent.EXTRA_TEXT, report);
            startActivity(Intent.createChooser(send, "Share crash report"));
        }));
        buttons.addView(button("Continue", v -> startGame()));
        root.addView(buttons);

        setContentView(root);
    }

    private Button button(String label, View.OnClickListener click) {
        Button b = new Button(this);
        b.setText(label);
        b.setAllCaps(false);
        b.setOnClickListener(click);
        return b;
    }
}
