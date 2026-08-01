//! Small pure helpers shared across display and alert paths.

/// Replace C0 controls + DEL for safe TTY free-text display.
///
/// Rule (M4.5 locked): U+0000–U+001F and U+007F → `'?'`; other UTF-8 left as-is.
/// Replacement preserves character-count intent (does not collapse hostile names
/// to empty solely because of controls).
pub fn sanitize_control_chars(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{0000}'..='\u{001F}' | '\u{007F}' => '?',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert_eq!(sanitize_control_chars(""), "");
    }

    #[test]
    fn only_controls_become_question_marks() {
        // NUL, BEL, LF, ESC, DEL
        let input = "\0\x07\n\x1b\x7f";
        let out = sanitize_control_chars(input);
        assert_eq!(out, "?????");
        assert_eq!(out.chars().count(), input.chars().count());
    }

    #[test]
    fn mixed_ansi_csi_sequence() {
        // ESC [ 3 1 m  red  ESC [ 0 m
        let input = "\x1b[31mnginx\x1b[0m";
        let out = sanitize_control_chars(input);
        assert!(!out.contains('\x1b'));
        assert!(out.starts_with("?[31mnginx"));
        assert!(out.contains("nginx"));
        // ESC twice → two '?'
        assert_eq!(out.matches('?').count(), 2);
    }

    #[test]
    fn printable_ascii_and_utf8_unchanged() {
        let s = "nginx-worker_1 café 日本語";
        assert_eq!(sanitize_control_chars(s), s);
    }

    #[test]
    fn del_and_bel_replaced() {
        assert_eq!(sanitize_control_chars("a\x7fb"), "a?b");
        assert_eq!(sanitize_control_chars("a\x07b"), "a?b");
    }

    #[test]
    fn preserves_char_count_for_controls() {
        let input = "\x00\x01\x02hello\x1f";
        let out = sanitize_control_chars(input);
        assert_eq!(out.chars().count(), input.chars().count());
        assert_eq!(out, "???hello?");
    }
}
