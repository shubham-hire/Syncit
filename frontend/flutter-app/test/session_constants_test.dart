import 'package:audiosync_app/features/session/session_constants.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('accepts 32-character hexadecimal identifiers', () {
    expect(isValidHex32('66f6ab42f1c79118ba5bacc069801b76'), isTrue);
    expect(isValidHex32('FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF'), isTrue);
  });

  test('rejects non-hex or incorrectly sized identifiers', () {
    expect(isValidHex32('66f6ab42f1c79118ba5bacc069801b7'), isFalse);
    expect(isValidHex32('66f6ab42f1c79118ba5bacc069801b7g'), isFalse);
    expect(isValidHex32('0x66f6ab42f1c79118ba5bacc069801b76'), isFalse);
  });
}
