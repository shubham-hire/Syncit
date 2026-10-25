import 'package:audiosync_app/engine/engine_controller.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const eventBody =
      'ListenerJoined { endpoint: 127.0.0.1:41000, '
      'listener_id: ListenerId([0, 1, 2, 3, 4, 5, 6, 7, '
      '8, 9, 10, 11, 12, 13, 14, 15]) }';

  test('parses a host listener event and rebuilds its hex id', () {
    final event = HostEvent.parse('{"event":"$eventBody"}');

    expect(event?.name, 'ListenerJoined');
    expect(event?.listenerHex, '000102030405060708090a0b0c0d0e0f');
  });

  test('accepts the legacy trailing-comma event payload', () {
    final event = HostEvent.parse('{"event":"$eventBody"},');

    expect(event?.name, 'ListenerJoined');
    expect(event?.listenerHex, '000102030405060708090a0b0c0d0e0f');
  });

  test('parses host diagnostics and listener packet metrics', () {
    final host = HostStatus.fromMap({
      'running': true,
      'events': ['ListenerApproved'],
      'errors': ['capture warning'],
    });
    expect(host.running, isTrue);
    expect(host.events, ['ListenerApproved']);
    expect(host.errors, ['capture warning']);

    final listener = ListenerStatus.fromMap({
      'state': 'Active',
      'active': true,
      'rendered': 3,
      'concealed': 4,
      'received': 10,
      'inserted': 9,
      'invalid': 1,
      'wrong_session': 2,
      'unauthenticated': 3,
      'duplicate': 4,
      'late': 5,
      'capacity_drops': 6,
      'sink_failures': 7,
      'watchdog_timeouts': 8,
      'rejoin_attempts': 9,
      'buffered': 2,
      'rtt_us': 90_000,
      'offset_us': -4_000,
      'output': 'Windows default output',
      'events': ['StateChanged(Active)'],
    });
    expect(listener.received, 10);
    expect(listener.inserted, 9);
    expect(listener.invalid, 1);
    expect(listener.wrongSession, 2);
    expect(listener.unauthenticated, 3);
    expect(listener.duplicate, 4);
    expect(listener.late, 5);
    expect(listener.capacityDrops, 6);
    expect(listener.sinkFailures, 7);
    expect(listener.watchdogTimeouts, 8);
    expect(listener.rejoinAttempts, 9);
    expect(listener.offsetMicros, -4_000);
  });

  test('preserves the host source label for the host UI', () {
    final info = HostSessionInfo.fromMap({
      'handle': 2,
      'session': '00112233445566778899aabbccddeeff',
      'token': 'ffeeddccbbaa99887766554433221100',
      'source': 'WASAPI loopback',
    });

    expect(info.source, 'WASAPI loopback');
  });
}
