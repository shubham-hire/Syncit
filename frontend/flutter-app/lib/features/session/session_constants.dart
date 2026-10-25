const mockSessionCode = 'AUDIO-7F2K';

final _hex32Pattern = RegExp(r'^[0-9a-fA-F]{32}$');

/// Returns whether [value] is exactly 16 bytes represented as hexadecimal.
///
/// Do not parse this as a Dart integer: session identifiers and tokens are
/// 128-bit values and may exceed the native integer range.
bool isValidHex32(String value) => _hex32Pattern.hasMatch(value);
