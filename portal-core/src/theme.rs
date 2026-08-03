use std::str::FromStr;

use serde::{Deserialize, Deserializer};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemeColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("theme color {value:?} must use exactly #RRGGBB")]
pub struct ThemeColorParseError {
    value: String,
}

impl ThemeColor {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

impl FromStr for ThemeColor {
    type Err = ThemeColorParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || ThemeColorParseError {
            value: value.to_owned(),
        };
        let hex = value.strip_prefix('#').ok_or_else(invalid)?;
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid());
        }

        Ok(Self::new(
            u8::from_str_radix(&hex[0..2], 16).map_err(|_| invalid())?,
            u8::from_str_radix(&hex[2..4], 16).map_err(|_| invalid())?,
            u8::from_str_radix(&hex[4..6], 16).map_err(|_| invalid())?,
        ))
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub accent: ThemeColor,
    pub muted: ThemeColor,
    pub signal: ThemeColor,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: ThemeColor::new(0x0b, 0x0f, 0x14),
            foreground: ThemeColor::new(0xd6, 0xde, 0xe8),
            accent: ThemeColor::new(0x7a, 0xa2, 0xf7),
            muted: ThemeColor::new(0x65, 0x71, 0x80),
            signal: ThemeColor::new(0x7f, 0xd8, 0x8f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, ThemeColor};

    #[test]
    fn default_palette_matches_the_stage_two_contract() {
        let theme = Theme::default();

        assert_eq!(theme.background, rgb(0x0b, 0x0f, 0x14));
        assert_eq!(theme.foreground, rgb(0xd6, 0xde, 0xe8));
        assert_eq!(theme.accent, rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(theme.muted, rgb(0x65, 0x71, 0x80));
        assert_eq!(theme.signal, rgb(0x7f, 0xd8, 0x8f));
    }

    #[test]
    fn parses_uppercase_and_lowercase_hex() {
        assert_eq!("#7aa2f7".parse(), Ok(rgb(0x7a, 0xa2, 0xf7)));
        assert_eq!("#7AA2F7".parse(), Ok(rgb(0x7a, 0xa2, 0xf7)));
    }

    #[test]
    fn rejects_invalid_shapes_and_characters() {
        for value in ["7AA2F7", "#7AF", "#7AA2F7FF", "#7AG2F7", " #7AA2F7"] {
            assert!(value.parse::<ThemeColor>().is_err(), "accepted {value:?}");
        }
    }

    const fn rgb(red: u8, green: u8, blue: u8) -> ThemeColor {
        ThemeColor { red, green, blue }
    }
}
