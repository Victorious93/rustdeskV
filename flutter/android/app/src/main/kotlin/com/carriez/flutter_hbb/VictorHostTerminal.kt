package com.carriez.flutter_hbb

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import android.provider.Settings
import com.termux.victor.shellbroker.IShellBroker
import com.termux.victor.shellbroker.IShellSessionCallback
import ffi.FFI
import java.security.SecureRandom

/**
 * rustdeskV as an Android terminal HOST: a rustdesk Terminal connection to this phone is served by VictorSuite's shell broker
 * (`com.termux` `ShellBrokerService`, API 1), never by a shell rustdeskV starts. The Rust side (`server/android_terminal_glue.rs`)
 * calls [MainService.rustVictorTerminal]; broker events go back through [FFI.onVictorTerminalEvent].
 *
 * Trust runs both ways: VictorSuite authorises rustdeskV by uid + pinned certificate (owner ticks rustdeskV for *Terminal* in
 * VictorSuite → Remote shell access), and rustdeskV binds only to a `com.termux` whose signing certificate the owner pinned in
 * rustdeskV ([VictorTerminalBridge.pinned]). The owner approves every session on the phone (the broker's dialog, bound to peer +
 * target + session + privilege). Privilege is always `user`. Nothing here logs or stores terminal bytes.
 */
object VictorHostTerminal {
    private const val BROKER_CLASS = "com.termux.victor.shellbroker.android.ShellBrokerService"
    private const val APPROVAL_WINDOW_MS = 120_000L

    @Volatile private var broker: IShellBroker? = null
    @Volatile private var bound = false
    private var appContext: Context? = null
    private val random = SecureRandom()

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) { broker = IShellBroker.Stub.asInterface(service) }
        override fun onServiceDisconnected(name: ComponentName?) { broker = null }
        override fun onBindingDied(name: ComponentName?) { broker = null; unbind(); }
    }

    /** Binds VictorSuite's broker if the owner pinned its certificate. Safe to call repeatedly. */
    @Synchronized
    fun ensureBound(ctx: Context) {
        appContext = ctx.applicationContext
        if (bound) return
        if (!pinMatches(ctx)) return
        bound = try {
            ctx.applicationContext.bindService(Intent().setComponent(ComponentName(VictorTerminalBridge.VICTORSUITE_PKG, BROKER_CLASS)), connection, Context.BIND_AUTO_CREATE)
        } catch (_: SecurityException) { false }
    }

    @Synchronized
    private fun unbind() {
        if (bound) { try { appContext?.unbindService(connection) } catch (_: Exception) { } }
        bound = false; broker = null
    }

    private fun pinMatches(ctx: Context): Boolean {
        val pin = VictorTerminalBridge.pinned(ctx) ?: return false
        return VictorTerminalBridge.certSha256(ctx, VictorTerminalBridge.VICTORSUITE_PKG).equals(pin, ignoreCase = true)
    }

    /** "true" only when the pinned VictorSuite is bound, speaks broker API >= 1 and has remote shell access switched on. */
    fun ready(ctx: Context): Boolean {
        ensureBound(ctx)
        if (!pinMatches(ctx)) { unbind(); return false }
        val b = broker ?: return false
        return try { b.apiVersion() >= 1 && b.capabilities().getBoolean("enabled", false) } catch (_: Exception) { false }
    }

    private fun target(ctx: Context): String {
        val name = try { Settings.Global.getString(ctx.contentResolver, Settings.Global.DEVICE_NAME) } catch (_: Exception) { null }
        return "phone:" + (name?.takeIf { it.isNotBlank() } ?: Build.MODEL)
    }

    /** Called from Rust. Returns "" or a stable error code. */
    fun handle(ctx: Context, op: String, sid: String, peer: String, a: Int, b: Int, data: ByteArray): String {
        val br = broker ?: return "unavailable"
        if (!pinMatches(ctx)) { unbind(); return "untrusted_cert" }
        return try {
            when (op) {
                "open" -> {
                    val nonce = ByteArray(16).also { random.nextBytes(it) }.joinToString("") { "%02x".format(it) }
                    val req = Bundle().apply {
                        putString("session_id", sid); putString("peer", peer); putString("target", target(ctx))
                        putString("mode", "interactive"); putString("privilege", "user")
                        putInt("cols", a); putInt("rows", b); putString("nonce", nonce)
                        putLong("expires_ms", System.currentTimeMillis() + APPROVAL_WINDOW_MS); putInt("caller_api_version", 1)
                    }
                    br.open(req, callback)
                }
                "input" -> br.input(sid, data)
                "resize" -> br.resize(sid, a, b)
                "close" -> br.close(sid, peer.ifBlank { "closed by rustdeskV" })
                else -> "bad_request"
            }
        } catch (_: Exception) { "unavailable" }
    }

    private val callback = object : IShellSessionCallback.Stub() {
        override fun onState(sessionId: String, state: String, reason: String?, binding: String?) =
            FFI.onVictorTerminalEvent(sessionId, "state", state, reason ?: "", ByteArray(0), 0)
        override fun onOutput(sessionId: String, data: ByteArray, channel: String?) =
            FFI.onVictorTerminalEvent(sessionId, "output", "", "", data, 0)
        override fun onExit(sessionId: String, status: Int) =
            FFI.onVictorTerminalEvent(sessionId, "exit", "", "", ByteArray(0), status)
    }
}
