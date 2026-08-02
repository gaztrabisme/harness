// dogfood/terax-keymap — shakedown #9 BLANKED STUB.
// Rebuild the three pure functions below from the spec in README.md. Each maps a
// terminal key event to the escape sequence a readline-style line editor expects,
// or null when the event is not a binding this function owns. Keep the signatures
// and names exactly as given (the frozen oracle imports them by name). Replace each
// `return null` body with the real mapping. This is JavaScript (no TypeScript): a
// key event is a plain object with boolean { altKey, ctrlKey, metaKey } and string
// { key, code } fields; platform is { isMac: boolean }.

/**
 * Word-wise cursor motion: Option/Alt + Left/Right arrow.
 * Active ONLY when Alt is the sole modifier (no Ctrl, no Meta) and the key is an
 * arrow. Left -> readline word-left (Esc b); Right -> readline word-right (Esc f).
 * Any other event -> null. Match the arrow on either `key` or `code`.
 */
export function terminalWordNavigationSequence(event) {
  return null;
}

/**
 * Line-edge cursor motion: Cmd/Meta + Left/Right arrow. macOS ONLY (null when not
 * mac). Active only when Meta is the sole modifier (no Alt, no Ctrl) and the key is
 * an arrow. Left -> line start (Ctrl+A); Right -> line end (Ctrl+E). Else null.
 * Match the arrow on either `key` or `code`.
 */
export function terminalLineNavigationSequence(event, opts) {
  return null;
}

/**
 * Modifier + Backspace deletion. The key must be Backspace (via `key` or `code`).
 *   macOS:  Cmd+Backspace    -> kill-to-line-start (Ctrl+U)
 *           Option+Backspace -> kill-word-backward (Ctrl+W)
 *   other:  Ctrl+Backspace   -> kill-word-backward (Ctrl+W)
 * Each binding fires only when its modifier is the SOLE modifier; every other
 * combination (and plain Backspace) -> null.
 */
export function terminalDeleteSequence(event, opts) {
  return null;
}
