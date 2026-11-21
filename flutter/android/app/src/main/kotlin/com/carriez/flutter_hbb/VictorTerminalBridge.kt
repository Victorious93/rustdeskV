package com.carriez.flutter_hbb

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.RemoteCallbackList
import com.termux.victor.shellbroker.IRemoteTerminalCallback
import io.flutter.plugin.common.MethodChannel
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * rustdeskV side of the VictorSuite terminal hand-off. rustdeskV owns the authenticated rustdesk connection to the exact peer; VictorSuite
 * (com.termux) owns the terminal UI. This object holds live sessions, the owner's pin of VictorSuite's signing certificate, and the glue to
 * the Dart session (MethodChannel). Nothing is logged or persisted except the pin; keystrokes are forwarded only while VictorSuite is
 * attached and never queued.
 */
object VictorTerminalBridge {
    const val VICTORSUITE_PKG = "com.termux"
    const val ACTIVITY = "com.termux.victor.shellbroker.android.RemoteTerminalActivity"
    private const val PREFS = "victor_terminal_bridge"

    class Session(val id: String, val peerId: String, val peerName: String) {
        @Volatile var callback: IRemoteTerminalCallback? = null
        @Volatile var cols = 80; @Volatile var rows = 24
        @Volatile var closed = false
    }

    val sessions = ConcurrentHashMap<String, Session>()
    private val waiting = ConcurrentHashMap<String, CountDownLatch>()      // peerId -> latch released when Dart registers the session
    private val opened = ConcurrentHashMap<String, String>()               // peerId -> session id
    var channel: MethodChannel? = null

    // ---------------------------------------------------------------------------------------------------- trust (owner-pinned)
    fun certSha256(ctx: Context, pkg: String): String? = try {
        @Suppress("DEPRECATION") val info = ctx.packageManager.getPackageInfo(pkg, PackageManager.GET_SIGNING_CERTIFICATES)
        val signers = info.signingInfo?.apkContentsSigners
        if (signers == null || signers.size != 1) null               // rotated/multiple signers: do not guess
        else MessageDigest.getInstance("SHA-256").digest(signers[0].toByteArray()).joinToString("") { "%02x".format(it) }
    } catch (e: Exception) { null }

    fun pinned(ctx: Context): String? = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString("victorsuite_cert", null)
    fun pin(ctx: Context, cert: String) { ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString("victorsuite_cert", cert.lowercase()).apply() }
    fun unpin(ctx: Context) { ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().remove("victorsuite_cert").apply(); sessions.values.forEach { end(it, "VictorSuite trust was removed") } }

    /** The calling uid must be exactly VictorSuite, signed with the certificate the owner pinned. */
    fun callerTrusted(ctx: Context, uid: Int): Boolean {
        val pkgs = ctx.packageManager.getPackagesForUid(uid) ?: return false
        if (pkgs.size != 1 || pkgs[0] != VICTORSUITE_PKG) return false
        val pin = pinned(ctx) ?: return false
        return certSha256(ctx, VICTORSUITE_PKG).equals(pin, ignoreCase = true)
    }

    // ---------------------------------------------------------------------------------------------------- sessions
    /** Dart opened a terminal session to [peerId] and wants VictorSuite to show it. Returns "" or an error code. */
    fun register(ctx: Context, sid: String, peerId: String, peerName: String, launch: Boolean): String {
        val pin = pinned(ctx)
        val cert = certSha256(ctx, VICTORSUITE_PKG) ?: return "not_installed"
        if (pin == null || !pin.equals(cert, true)) return "not_trusted"
        val s = Session(sid, peerId, peerName); sessions[sid] = s; opened[peerId] = sid; waiting.remove(peerId)?.countDown()
        if (launch) ctx.startActivity(Intent().setComponent(ComponentName(VICTORSUITE_PKG, ACTIVITY)).putExtra("session_id", sid).putExtra("peer_id", peerId)
            .putExtra("peer_name", peerName).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        return ""
    }

    /** VictorSuite → Remote shell asked for [peerId]: bring rustdeskV's UI up to connect (the Dart side registers the session), then wait. */
    fun openPeer(ctx: Context, peerId: String): String {
        val latch = CountDownLatch(1); waiting[peerId] = latch
        ctx.startActivity(Intent(ctx, MainActivity::class.java).setAction("com.carriez.flutter_hbb.VICTORSUITE_TERMINAL").putExtra("peer_id", peerId).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        if (!latch.await(45, TimeUnit.SECONDS)) { waiting.remove(peerId); return "error:timeout:the peer did not open a terminal (offline, refused or login needed)" }
        return opened[peerId]?.let { "session:$it" } ?: "error:unavailable:no session"
    }

    fun expectingPeer(peerId: String) = waiting.containsKey(peerId)

    fun output(sid: String, data: ByteArray) { sessions[sid]?.takeIf { !it.closed }?.callback?.let { try { it.onOutput(sid, data) } catch (_: Exception) { } } }
    fun exited(sid: String, status: Int) { sessions[sid]?.callback?.let { try { it.onExit(sid, status) } catch (_: Exception) { } }; sessions[sid]?.let { end(it, "peer closed the terminal") } }
    fun end(s: Session, why: String) {
        if (s.closed) return; s.closed = true; sessions.remove(s.id)
        s.callback?.let { try { it.onState(s.id, "closed", why) } catch (_: Exception) { } }
        channel?.invokeMethod("victor_terminal_close", mapOf("sid" to s.id))
    }

    fun listPeers(ctx: Context, peers: List<Map<String, Any?>>): Bundle = Bundle().apply {
        putParcelableArrayList("peers", ArrayList(peers.map { p -> Bundle().apply {
            putString("peer_id", p["id"] as? String); putString("name", p["name"] as? String ?: p["id"] as? String); putString("platform", p["platform"] as? String)
            putBoolean("terminal_ok", (p["platform"] as? String)?.equals("Linux", true) == true)          // VictorLinux PCs
        } }))
    }
}
