use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!(
            "#{:02X}{:02X}{:02X}",
            self.red, self.green, self.blue
        ))
    }
}

impl Theme {
    #[must_use]
    pub const fn portal() -> Self {
        Self {
            background: ThemeColor::new(0x0b, 0x0f, 0x14),
            foreground: ThemeColor::new(0xd6, 0xde, 0xe8),
            accent: ThemeColor::new(0x7a, 0xa2, 0xf7),
            muted: ThemeColor::new(0x65, 0x71, 0x80),
            signal: ThemeColor::new(0x7f, 0xd8, 0x8f),
        }
    }

    #[must_use]
    pub const fn hacker() -> Self {
        Self {
            background: ThemeColor::new(0x03, 0x08, 0x05),
            foreground: ThemeColor::new(0x00, 0xff, 0x41),
            accent: ThemeColor::new(0x00, 0xc8, 0x53),
            muted: ThemeColor::new(0x1b, 0x5e, 0x20),
            signal: ThemeColor::new(0x7c, 0xff, 0x6b),
        }
    }

    #[must_use]
    pub const fn cyber_cyan() -> Self {
        Self {
            background: ThemeColor::new(0x05, 0x0a, 0x12),
            foreground: ThemeColor::new(0xc9, 0xf7, 0xff),
            accent: ThemeColor::new(0x00, 0xe5, 0xff),
            muted: ThemeColor::new(0x3c, 0x6a, 0x73),
            signal: ThemeColor::new(0x5f, 0xfb, 0xf1),
        }
    }

    #[must_use]
    pub const fn terminator() -> Self {
        Self {
            background: ThemeColor::new(0x0a, 0x00, 0x00),
            foreground: ThemeColor::new(0xff, 0x4a, 0x3d),
            accent: ThemeColor::new(0xff, 0x17, 0x17),
            muted: ThemeColor::new(0x70, 0x20, 0x20),
            signal: ThemeColor::new(0xff, 0x9a, 0x8f),
        }
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
        Self::portal()
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
    fn preset_palettes_are_distinct_and_cohesive() {
        let portal = Theme::portal();
        let hacker = Theme::hacker();
        let cyber = Theme::cyber_cyan();
        let terminator = Theme::terminator();

        assert_eq!(portal, Theme::default());
        assert_ne!(hacker, portal);
        assert_ne!(cyber, portal);
        assert_ne!(hacker, cyber);
        assert_ne!(terminator, portal);
        assert_ne!(terminator, hacker);
        assert_ne!(terminator, cyber);
        assert_eq!(hacker.foreground, rgb(0x00, 0xff, 0x41));
        assert_eq!(cyber.accent, rgb(0x00, 0xe5, 0xff));
        assert_eq!(terminator.accent, rgb(0xff, 0x17, 0x17));
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
