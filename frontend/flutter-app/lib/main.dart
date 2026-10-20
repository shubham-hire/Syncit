import 'package:flutter/material.dart';

import 'app.dart';

void main() {
  runApp(const SyncItApp());
}

// Keep the old public symbol available to existing callers while the UI is
// migrated to the SyncIt product name.
typedef AudioSyncApp = SyncItApp;
