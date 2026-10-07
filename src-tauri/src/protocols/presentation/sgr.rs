//! Atomic SGR interpretation over the decoder's already bounded parameter bytes.
use super::{TextColor, TextStyle, basic_color};

// Saturation classifies arbitrarily long numbers as unsupported without overflow.
fn number(bytes: &[u8], empty_is_zero: bool) -> Option<u16> {
    if bytes.is_empty() && !empty_is_zero {
        return None;
    }
    bytes.iter().try_fold(0_u16, |value, byte| {
        byte.is_ascii_digit()
            .then(|| (value * 10 + u16::from(byte - b'0')).min(256))
    })
}
fn component(bytes: &[u8]) -> Option<u8> {
    u8::try_from(number(bytes, false)?).ok()
}
fn indexed(index: u8) -> TextColor {
    if index < 8 {
        basic_color(u16::from(index))
    } else {
        TextColor::Indexed(index)
    }
}
fn rgb(red: &[u8], green: &[u8], blue: &[u8]) -> Option<TextColor> {
    Some(TextColor::Rgb {
        red: component(red)?,
        green: component(green)?,
        blue: component(blue)?,
    })
}
fn colon(field: &[u8]) -> Option<(u16, TextColor)> {
    let mut fields = field.split(|&byte| byte == b':');
    let target = number(fields.next()?, false)?;
    if target != 38 && target != 48 {
        return None;
    }
    let color = match number(fields.next()?, false)? {
        5 => indexed(component(fields.next()?)?),
        2 => {
            let first = fields.next()?;
            let second = fields.next()?;
            let third = fields.next()?;
            if let Some(fourth) = fields.next() {
                // Only the default RGB space is supported; no color-space conversion.
                if number(first, true)? != 0 {
                    return None;
                }
                rgb(second, third, fourth)?
            } else {
                rgb(first, second, third)?
            }
        }
        _ => return None,
    };
    if fields.next().is_some() {
        return None;
    }
    Some((target, color))
}
fn set_color(style: &mut TextStyle, target: u16, color: TextColor) {
    if target == 38 {
        style.foreground = color;
    } else {
        style.background = color;
    }
}
pub(super) fn apply(style: TextStyle, parameters: &[u8]) -> Option<TextStyle> {
    let mut next = style;
    let mut fields = parameters.split(|&byte| byte == b';');
    while let Some(field) = fields.next() {
        if field.contains(&b':') {
            let (target, color) = colon(field)?;
            set_color(&mut next, target, color);
            continue;
        }
        match number(field, true)? {
            0 => next = TextStyle::default(),
            1 => next.bold = true,
            3 => next.italic = true,
            4 => next.underline = true,
            7 => next.inverse = true,
            22 => next.bold = false,
            23 => next.italic = false,
            24 => next.underline = false,
            27 => next.inverse = false,
            value @ 30..=37 => next.foreground = basic_color(value - 30),
            39 => next.foreground = TextColor::Default,
            value @ 40..=47 => next.background = basic_color(value - 40),
            49 => next.background = TextColor::Default,
            value @ 90..=97 => next.foreground = indexed((value - 90 + 8) as u8),
            value @ 100..=107 => next.background = indexed((value - 100 + 8) as u8),
            target @ (38 | 48) => {
                let color = match number(fields.next()?, false)? {
                    5 => indexed(component(fields.next()?)?),
                    2 => rgb(fields.next()?, fields.next()?, fields.next()?)?,
                    _ => return None,
                };
                set_color(&mut next, target, color);
            }
            _ => return None,
        }
    }
    Some(next)
}
