import 'dart:async';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/terminal_model.dart';

/// VictorSuite terminal hand-off (Android). rustdeskV keeps the authenticated connection to the exact peer; VictorSuite draws the terminal.
/// This build never draws its own terminal on Android: if the hand-off is not possible the page says why and stops (no fallback), which is
/// also why it declares `victorsuite/1` in its Terminal login (hosts refuse phone clients that do not).
class VictorSuiteTerminal {
  static const MethodChannel _ch = MethodChannel('victorsuite_terminal');
  static final Map<String, _Handle> _sessions = {};
  static void Function(String peerId)? onOpenRequest;
  static bool _wired = false;

  static void _wire() {
    if (_wired) return;
    _wired = true;
    _ch.setMethodCallHandler((call) async {
      final a = (call.arguments as Map?) ?? const {};
      final sid = a['sid'] as String?;
      switch (call.method) {
        case 'victor_terminal_input':
          final h = _sessions[sid];
          if (h != null) await h.model.sendRemoteInput(Uint8List.fromList(List<int>.from(a['data'] as List)));
          break;
        case 'victor_terminal_resize':
          final h = _sessions[sid];
          if (h != null) await h.model.resizeRemote(a['cols'] as int, a['rows'] as int);
          break;
        case 'victor_terminal_close':
          _sessions[sid]?.onClose();
          _sessions.remove(sid);
          break;
        case 'open_terminal':
          onOpenRequest?.call(a['peer_id'] as String);
          break;
      }
      return null;
    });
  }

  /// {installed, cert, pinned} for VictorSuite (`com.termux`).
  static Future<Map<String, dynamic>> candidate() async {
    _wire();
    return Map<String, dynamic>.from(await _ch.invokeMethod('victor_terminal_candidate') as Map);
  }

  static Future<void> trust(String cert) => _ch.invokeMethod('victor_terminal_trust', {'cert': cert});

  /// Pushes the peer list VictorSuite's "Remote shell" shows (id, name, platform).
  static Future<void> publishPeers(List<Map<String, String>> peers) => _ch.invokeMethod('victor_terminal_peers', peers);

  /// Connects [model] (an already-created, not yet drawn terminal model) to VictorSuite. Returns "" or an error code
  /// (`not_installed`, `not_trusted`, ...). Output flows to VictorSuite; input/resize/close come back from it.
  static Future<String> attach({
    required String sid,
    required String peerId,
    required String peerName,
    required TerminalModel model,
    required VoidCallback onClose,
    bool launch = true,
  }) async {
    _wire();
    model.onRemoteOutput = (b) => _ch.invokeMethod('victor_terminal_output', {'sid': sid, 'data': b});
    model.onRemoteExit = (c) => _ch.invokeMethod('victor_terminal_exit', {'sid': sid, 'status': c});
    _sessions[sid] = _Handle(model, onClose);
    final r = await _ch.invokeMethod('victor_terminal_register', {'sid': sid, 'peer_id': peerId, 'peer_name': peerName, 'launch': launch}) as String;
    if (r.isNotEmpty) {
      _sessions.remove(sid);
      model.onRemoteOutput = null;
    }
    return r;
  }

  static String explain(String code) {
    switch (code) {
      case 'not_installed':
        return 'VictorSuite is not installed. Install it to use Terminal on this phone.';
      case 'not_trusted':
        return 'VictorSuite is not trusted by rustdeskV yet. Confirm its signing certificate to continue.';
      default:
        return 'The terminal could not be handed to VictorSuite ($code).';
    }
  }
}

class _Handle {
  _Handle(this.model, this.onClose);
  final TerminalModel model;
  final VoidCallback onClose;
}

/// Shown instead of an xterm view on Android.
class VictorSuiteHandoffView extends StatelessWidget {
  const VictorSuiteHandoffView({super.key, required this.status, required this.peer, this.error, this.onTrust, required this.onClose});
  final String status;
  final String peer;
  final String? error;
  final VoidCallback? onTrust;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: Text('Terminal — $peer')),
        body: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(error ?? status, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            const Text('The terminal is shown by VictorSuite. This connection stays in rustdeskV: no keystrokes or output are stored here.'),
            const SizedBox(height: 24),
            Row(children: [
              if (onTrust != null) ElevatedButton(onPressed: onTrust, child: const Text('Review VictorSuite certificate…')),
              const SizedBox(width: 12),
              TextButton(onPressed: onClose, child: const Text('Close')),
            ]),
          ]),
        ),
      );
}
