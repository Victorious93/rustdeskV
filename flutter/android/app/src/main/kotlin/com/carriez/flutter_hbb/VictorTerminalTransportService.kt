package com.carriez.flutter_hbb

import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.Bundle
import android.os.IBinder
import com.termux.victor.shellbroker.IRemoteTerminalCallback
import com.termux.victor.shellbroker.IRemoteTerminalTransport

/**
 * Transport provider for VictorSuite's Remote shell. Exported so VictorSuite can bind it; every call is authorised by
 * [Binder.getCallingUid] → package `com.termux` → the signing certificate the owner pinned in rustdeskV (see VictorTerminalBridge). Anything
 * else gets "not_trusted". Data flows between the Dart rustdesk session and VictorSuite's terminal view; rustdeskV never logs it.
 */
class VictorTerminalTransportService : Service() {
    override fun onBind(intent: Intent?): IBinder = stub

    private fun trusted() = VictorTerminalBridge.callerTrusted(applicationContext, Binder.getCallingUid())
    private fun dart(method: String, args: Map<String, Any?>) { MainActivity.flutterMethodChannel?.let { ch -> android.os.Handler(mainLooper).post { ch.invokeMethod(method, args) } } }

    private val stub = object : IRemoteTerminalTransport.Stub() {
        override fun apiVersion() = 1

        override fun listPeers(): Bundle {
            if (!trusted()) return Bundle().apply { putString("error", "not_trusted") }
            return VictorTerminalBridge.listPeers(applicationContext, VictorTerminalPeers.snapshot())
        }

        override fun openPeer(peerId: String, cols: Int, rows: Int): String =
            if (!trusted()) "error:not_trusted:trust VictorSuite in rustdeskV first" else VictorTerminalBridge.openPeer(applicationContext, peerId)

        override fun attach(sessionId: String, callback: IRemoteTerminalCallback): String {
            if (!trusted()) return "not_trusted"
            val s = VictorTerminalBridge.sessions[sessionId] ?: return "no_such_session"
            if (s.callback != null) return "already_attached"          // one viewer; a second app cannot hijack the session
            s.callback = callback
            try { callback.onState(sessionId, "open", null) } catch (_: Exception) { }
            return ""
        }

        override fun input(sessionId: String, data: ByteArray): String {
            if (!trusted()) return "not_trusted"
            val s = VictorTerminalBridge.sessions[sessionId]?.takeIf { !it.closed && it.callback != null } ?: return "no_such_session"
            dart("victor_terminal_input", mapOf("sid" to s.id, "data" to data))          // forwarded now or dropped; never queued
            return ""
        }

        override fun resize(sessionId: String, cols: Int, rows: Int): String {
            if (!trusted()) return "not_trusted"
            val s = VictorTerminalBridge.sessions[sessionId] ?: return "no_such_session"
            s.cols = cols; s.rows = rows; dart("victor_terminal_resize", mapOf("sid" to s.id, "cols" to cols, "rows" to rows)); return ""
        }

        override fun close(sessionId: String): String {
            if (!trusted()) return "not_trusted"
            VictorTerminalBridge.sessions[sessionId]?.let { VictorTerminalBridge.end(it, "closed by VictorSuite") }; return ""
        }
    }
}

/** Latest peer list pushed by Dart (id, name, platform). Not persisted. */
object VictorTerminalPeers {
    @Volatile private var peers: List<Map<String, Any?>> = emptyList()
    fun update(p: List<Map<String, Any?>>) { peers = p }
    fun snapshot() = peers
}
