import 'package:audiosync_app/app.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('home presents host and listener roles', (tester) async {
    await tester.pumpWidget(const SyncItApp());

    expect(find.text('SyncIt'), findsOneWidget);
    expect(find.text('Host a session'), findsOneWidget);
    expect(find.text('Listen to a session'), findsOneWidget);
  });

  testWidgets('host can start a mock session and add a listener', (
    tester,
  ) async {
    await tester.pumpWidget(const SyncItApp());
    await tester.tap(find.text('Create session'));
    await tester.pumpAndSettle();

    expect(find.text('Start session'), findsOneWidget);
    await tester.tap(find.text('Start session'));
    await tester.pumpAndSettle();

    expect(find.text('Session is ready'), findsOneWidget);
    expect(find.text('AUDIO-7F2K'), findsOneWidget);
    await tester.tap(find.text('Simulate listener'));
    await tester.pump();
    expect(find.text('Listener 1'), findsOneWidget);
    expect(find.text('Awaiting host approval'), findsOneWidget);
    await tester.tap(find.byTooltip('Approve listener'));
    await tester.pump();
    expect(find.text('Approved · mock transport'), findsOneWidget);
    await tester.tap(find.byTooltip('Remove listener'));
    await tester.pump();
    expect(find.text('No listeners have joined yet.'), findsOneWidget);
  });

  testWidgets('listener can join and adjust playback controls', (tester) async {
    await tester.pumpWidget(const SyncItApp());
    await tester.tap(find.text('Join session'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'AUDIO-7F2K');
    await tester.tap(find.text('Join session'));
    await tester.pumpAndSettle();

    expect(find.text('Mock session connected'), findsOneWidget);
    expect(find.text('Delay 0 ms'), findsOneWidget);
    expect(find.text('Volume 80%'), findsOneWidget);
  });
}
