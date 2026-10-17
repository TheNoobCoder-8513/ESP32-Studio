/// Formats a GPIO pin state change command for the ESP32.
/// Format: "<pin>:on\n" or "<pin>:off\n" (e.g., "23:on\n").
pub fn format_pin_command(pin: i32, status: bool) -> String {
    let state = if status { "on" } else { "off" };
    format!("{}:{}\n", pin, state)
}

/// Normalizes raw command input to ensure it ends with a newline.
pub fn format_raw_command(input: &str) -> String {
    let trimmed = input.trim();
    format!("{}\n", trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_pin_command_on() {
        assert_eq!(format_pin_command(23, true), "23:on\n");
    }

    #[test]
    fn test_format_pin_command_off() {
        assert_eq!(format_pin_command(2, false), "2:off\n");
    }

    #[test]
    fn test_format_raw_command() {
        assert_eq!(format_raw_command("hello  "), "hello\n");
        assert_eq!(format_raw_command("  16:on\r\n"), "16:on\n");
    }
}

