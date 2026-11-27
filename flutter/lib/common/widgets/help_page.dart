// RustDeskV in-app Help: shows the bundled user manual (assets/MANUAL.md).
//
// The manual is rendered as plain selectable text on purpose, so no markdown
// package has to be added as a dependency. The canonical copy lives in
// docs/MANUAL.md; flutter/assets/MANUAL.md must be kept identical.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;

import '../../consts.dart';

const String kRustDeskVManualAsset = 'assets/MANUAL.md';

/// Opens the RustDeskV manual as a full page.
void showRustDeskVHelp(BuildContext context) {
  Navigator.of(context).push(
    MaterialPageRoute(builder: (_) => const RustDeskVHelpPage()),
  );
}

class RustDeskVHelpPage extends StatelessWidget {
  const RustDeskVHelpPage({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('RustDeskV Help')),
      body: FutureBuilder<String>(
        future: rootBundle.loadString(kRustDeskVManualAsset),
        builder: (context, snapshot) {
          if (snapshot.connectionState != ConnectionState.done) {
            return const Center(child: CircularProgressIndicator());
          }
          final text = snapshot.data;
          if (snapshot.hasError || text == null || text.isEmpty) {
            return Padding(
              padding: const EdgeInsets.all(16),
              child: SelectableText(
                'The manual could not be loaded.\n\n'
                'Read it online: $kRustDeskVManualUrl',
              ),
            );
          }
          return Scrollbar(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(16),
              child: SelectableText(
                text,
                style: const TextStyle(fontSize: 15, height: 1.4),
              ),
            ),
          );
        },
      ),
    );
  }
}
