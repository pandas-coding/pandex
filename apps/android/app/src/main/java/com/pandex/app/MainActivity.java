package com.pandex.app;

import android.app.Activity;
import android.content.res.Configuration;
import android.os.Build;
import android.os.Bundle;
import android.view.Choreographer;
import android.view.SurfaceControl;
import android.view.SurfaceControlInputReceiver;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsetsController;
import android.view.WindowManager;

import com.longbridge.nsh.NshView;

/**
 * M0 spike host (docs/m0-spike.md gates 2-3): the window is FLAG_SECURE and
 * the Rust app runs in an embedded NshView. Mirrors nsh's AppActivity using
 * only public embedding APIs; the edge-to-edge and system-bar helpers are
 * nsh DevActivity's, which are package-private there.
 */
public final class MainActivity extends Activity {
    private NshView view;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        // Gate 2: no screenshots, screen recordings or recents thumbnails.
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        edgeToEdge(this);
        view = new NshView(this);
        setContentView(view);
        view.handleNotificationIntent(getIntent());
        if (state == null) view.handleUrlSchemeIntent(getIntent());
        view.runRustApp("nsh");
    }

    @Override
    public void onConfigurationChanged(Configuration config) {
        super.onConfigurationChanged(config);
        systemBars(this);
    }

    @Override
    protected void onNewIntent(android.content.Intent intent) {
        super.onNewIntent(intent);
        if (view != null) {
            view.handleNotificationIntent(intent);
            view.handleUrlSchemeIntent(intent);
        }
    }

    /** Android 12 and older: the app gets the back button first. */
    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (view == null || !view.handleBack()) super.onBackPressed();
    }

    @Override
    protected void onDestroy() {
        view.destroy();
        super.onDestroy();
    }

    /**
     * NSH dialogs and popups are separate windows added through this
     * Activity's WindowManager (Dialog/PopupWindow take it from
     * getSystemService); the activity's own FLAG_SECURE does not reach
     * them. Force it on every window added here (gate 2 leak fix).
     */
    @Override
    public Object getSystemService(String name) {
        if (WINDOW_SERVICE.equals(name)) {
            if (secureWindowManager == null) {
                secureWindowManager = new SecureWindowManager(
                        (WindowManager) super.getSystemService(name));
            }
            return secureWindowManager;
        }
        return super.getSystemService(name);
    }

    private SecureWindowManager secureWindowManager;

    private static final class SecureWindowManager implements WindowManager {
        private final WindowManager delegate;

        SecureWindowManager(WindowManager delegate) {
            this.delegate = delegate;
        }

        private static ViewGroup.LayoutParams secure(ViewGroup.LayoutParams params) {
            if (params instanceof WindowManager.LayoutParams) {
                ((WindowManager.LayoutParams) params).flags |= WindowManager.LayoutParams.FLAG_SECURE;
            }
            return params;
        }

        @Override
        public void addView(View view, ViewGroup.LayoutParams params) {
            delegate.addView(view, secure(params));
        }

        @Override
        public void updateViewLayout(View view, ViewGroup.LayoutParams params) {
            delegate.updateViewLayout(view, secure(params));
        }

        @Override
        public void removeView(View view) {
            delegate.removeView(view);
        }

        @Override
        public void removeViewImmediate(View view) {
            delegate.removeViewImmediate(view);
        }

        @Override
        @SuppressWarnings("deprecation")
        public android.view.Display getDefaultDisplay() {
            return delegate.getDefaultDisplay();
        }

        // The interface's default methods throw UnsupportedOperationException;
        // Dialog/PopupWindow reach some of them (createLocalWindowManager is
        // hidden but its signature types are public, so it can be overridden).
        // Delegate everything.

        // Hidden in the SDK stub (and reflection is hidden-API blocked).
        // The local manager binds sub-windows to their parent; a dialog's
        // own decor carries its token already, so a fresh wrapper over the
        // same delegate keeps FLAG_SECURE on everything added through it.
        public WindowManager createLocalWindowManager(Window parentWindow) {
            return new SecureWindowManager(delegate);
        }

        public WindowManager createPresentationWindowManager(android.view.Display display) {
            return new SecureWindowManager(delegate);
        }

        @Override
        public android.view.WindowMetrics getCurrentWindowMetrics() {
            return delegate.getCurrentWindowMetrics();
        }

        @Override
        public android.view.WindowMetrics getMaximumWindowMetrics() {
            return delegate.getMaximumWindowMetrics();
        }

        @Override
        public boolean isCrossWindowBlurEnabled() {
            return delegate.isCrossWindowBlurEnabled();
        }

        @Override
        public void addCrossWindowBlurEnabledListener(java.util.function.Consumer<Boolean> listener) {
            delegate.addCrossWindowBlurEnabledListener(listener);
        }

        @Override
        public void addCrossWindowBlurEnabledListener(java.util.concurrent.Executor executor,
                java.util.function.Consumer<Boolean> listener) {
            delegate.addCrossWindowBlurEnabledListener(executor, listener);
        }

        @Override
        public void removeCrossWindowBlurEnabledListener(java.util.function.Consumer<Boolean> listener) {
            delegate.removeCrossWindowBlurEnabledListener(listener);
        }

        @Override
        public void addProposedRotationListener(java.util.concurrent.Executor executor,
                java.util.function.IntConsumer listener) {
            delegate.addProposedRotationListener(executor, listener);
        }

        @Override
        public void removeProposedRotationListener(java.util.function.IntConsumer listener) {
            delegate.removeProposedRotationListener(listener);
        }

        @Override
        public void registerTrustedPresentationListener(android.os.IBinder window,
                android.window.TrustedPresentationThresholds thresholds,
                java.util.concurrent.Executor executor,
                java.util.function.Consumer<Boolean> listener) {
            delegate.registerTrustedPresentationListener(window, thresholds, executor, listener);
        }

        @Override
        public void unregisterTrustedPresentationListener(java.util.function.Consumer<Boolean> listener) {
            delegate.unregisterTrustedPresentationListener(listener);
        }

        @Override
        public android.window.InputTransferToken registerBatchedSurfaceControlInputReceiver(
                android.window.InputTransferToken inputTransferToken, SurfaceControl surfaceControl,
                Choreographer choreographer, SurfaceControlInputReceiver receiver) {
            return delegate.registerBatchedSurfaceControlInputReceiver(
                    inputTransferToken, surfaceControl, choreographer, receiver);
        }

        @Override
        public android.window.InputTransferToken registerUnbatchedSurfaceControlInputReceiver(
                android.window.InputTransferToken inputTransferToken, SurfaceControl surfaceControl,
                android.os.Looper looper, SurfaceControlInputReceiver receiver) {
            return delegate.registerUnbatchedSurfaceControlInputReceiver(
                    inputTransferToken, surfaceControl, looper, receiver);
        }

        @Override
        public void unregisterSurfaceControlInputReceiver(SurfaceControl surfaceControl) {
            delegate.unregisterSurfaceControlInputReceiver(surfaceControl);
        }

        @Override
        public boolean transferTouchGesture(android.window.InputTransferToken from,
                android.window.InputTransferToken to) {
            return delegate.transferTouchGesture(from, to);
        }

        @Override
        public int addScreenRecordingCallback(java.util.concurrent.Executor executor,
                java.util.function.Consumer<Integer> callback) {
            return delegate.addScreenRecordingCallback(executor, callback);
        }

        @Override
        public void removeScreenRecordingCallback(java.util.function.Consumer<Integer> callback) {
            delegate.removeScreenRecordingCallback(callback);
        }
    }

    /** Draw behind the system bars: the app gets them as `safeArea()`. */
    @SuppressWarnings("deprecation")
    private static void edgeToEdge(Activity activity) {
        Window window = activity.getWindow();
        if (Build.VERSION.SDK_INT >= 30) {
            window.setDecorFitsSystemWindows(false);
        } else {
            window.getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
        window.setStatusBarColor(0);
        window.setNavigationBarColor(0);
        systemBars(activity);
    }

    /** Light system bar icons over dark content, dark ones over light. */
    @SuppressWarnings("deprecation")
    private static void systemBars(Activity activity) {
        int night = activity.getResources().getConfiguration().uiMode
                & Configuration.UI_MODE_NIGHT_MASK;
        boolean dark = night == Configuration.UI_MODE_NIGHT_YES;
        Window window = activity.getWindow();
        window.getDecorView();
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController c = window.getInsetsController();
            if (c == null) return;
            int light = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                    | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
            c.setSystemBarsAppearance(dark ? 0 : light, light);
        } else {
            View decor = window.getDecorView();
            int flags = decor.getSystemUiVisibility();
            int light = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
                    | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
            decor.setSystemUiVisibility(dark ? flags & ~light : flags | light);
        }
    }
}
