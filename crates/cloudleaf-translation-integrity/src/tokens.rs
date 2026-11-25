use crate::{FindingKind, FindingStatus};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Currency {
    Cny,
    Usd,
    Cad,
    Aud,
    Nzd,
    Hkd,
    Sgd,
    Twd,
    Jpy,
    Eur,
    Gbp,
    DollarSymbol,
    YenSymbol,
}

impl Currency {
    pub(crate) fn is_ambiguous(self) -> bool {
        matches!(self, Self::DollarSymbol | Self::YenSymbol)
    }

    pub(crate) fn may_be(self, known: Self) -> bool {
        match self {
            Self::DollarSymbol => matches!(
                known,
                Self::Usd | Self::Cad | Self::Aud | Self::Nzd | Self::Hkd | Self::Sgd | Self::Twd
            ),
            Self::YenSymbol => matches!(known, Self::Cny | Self::Jpy),
            currency => currency == known,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Unit {
    Celsius,
    Fahrenheit,
    Degree,
    Unclear,
    Millimetre,
    Centimetre,
    Metre,
    Kilometre,
    Inch,
    Foot,
    Mile,
    Milligram,
    Gram,
    Kilogram,
    Tonne,
    Pound,
    Ounce,
    Millilitre,
    Litre,
    FluidOunce,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Value {
    Number(String),
    Percentage(String),
    Money(String, Currency),
    Unit(String, Unit),
    Date(u32, u32, u32),
    Time(u32, u32, u32),
    Uncertain(FindingKind, FindingStatus),
}

impl Value {
    pub(crate) fn kind(&self) -> FindingKind {
        match self {
            Self::Number(_) => FindingKind::Number,
            Self::Percentage(_) => FindingKind::Percentage,
            Self::Money(..) => FindingKind::Money,
            Self::Unit(..) => FindingKind::Unit,
            Self::Date(..) => FindingKind::Date,
            Self::Time(..) => FindingKind::Time,
            Self::Uncertain(kind, _) => *kind,
        }
    }

    pub(crate) fn number(&self) -> Option<&str> {
        match self {
            Self::Number(number)
            | Self::Percentage(number)
            | Self::Money(number, _)
            | Self::Unit(number, _) => Some(number),
            _ => None,
        }
    }

    pub(crate) fn uncertainty(&self) -> Option<FindingStatus> {
        match self {
            Self::Money(_, currency) if currency.is_ambiguous() => Some(FindingStatus::Ambiguous),
            Self::Unit(_, Unit::Degree | Unit::Unclear) => Some(FindingStatus::Ambiguous),
            Self::Uncertain(_, status) => Some(*status),
            _ => None,
        }
    }
}

pub(crate) struct Token {
    pub(crate) span: Range<usize>,
    pub(crate) value: Value,
}

pub(crate) fn tokenize(text: &str, protected: &[Range<usize>]) -> Vec<Token> {
    let mut rows = Vec::new();
    let mut spans = protected.iter().peekable();
    let mut cursor = 0;
    while cursor < text.len() {
        while spans.peek().is_some_and(|span| span.end <= cursor) {
            spans.next();
        }
        let remaining = &text[cursor..];
        let character = remaining.chars().next().expect("下一个匹配位置存在");
        let previous = text[..cursor].chars().next_back();
        if !previous.is_some_and(ascii_identifier)
            && let Some((len, value)) = calendar(remaining)
                .or_else(|| clock(remaining))
                .or_else(|| numeric(remaining))
        {
            let end = cursor + len;
            // 数字可以是型号的一部分。货币、日期等独立事实仍需要检查。
            let term_number = value.kind() == FindingKind::Number
                && spans.peek().is_some_and(|span| {
                    span.start <= cursor && end <= span.end && span.len() > len
                });
            if !term_number {
                rows.push(Token {
                    span: cursor..end,
                    value,
                });
            }
            cursor = end;
        } else {
            cursor += character.len_utf8();
        }
    }
    rows
}

fn calendar(text: &str) -> Option<(usize, Value)> {
    if let Some((month_len, month)) = month_name(text) {
        let rest = text[month_len..].trim_start();
        let (day_len, day) = integer(rest, 1, 2)?;
        let year_text = rest[day_len..].trim_start();
        let year_text = year_text
            .strip_prefix(',')
            .unwrap_or(year_text)
            .trim_start();
        let (year_len, year) = integer(year_text, 4, 4)?;
        return Some((
            text.len() - year_text.len() + year_len,
            date(year, month, day),
        ));
    }

    let (first_len, first) = integer(text, 1, 4)?;
    let rest = &text[first_len..];
    let first_digits = text[..first_len].chars().count();
    let separator = rest.chars().next()?;
    if first_digits == 4 && rest.trim_start().starts_with('年') {
        let month_text = rest.trim_start().strip_prefix('年')?.trim_start();
        let (month_len, month) = integer(month_text, 1, 2)?;
        let day_text = month_text[month_len..]
            .trim_start()
            .strip_prefix('月')?
            .trim_start();
        let (day_len, day) = integer(day_text, 1, 2)?;
        let day_marker = day_text[day_len..].chars().next()?;
        if !matches!(day_marker, '日' | '号') {
            return None;
        }
        return Some((
            text.len() - day_text.len() + day_len + day_marker.len_utf8(),
            date(first, month, day),
        ));
    }
    if matches!(separator, '-' | '/' | '－' | '／') {
        let second_text = &rest[separator.len_utf8()..];
        let (second_len, second) = integer(second_text, 1, 2)?;
        let third_text = second_text[second_len..].strip_prefix(separator)?;
        let year_first = first_digits == 4;
        let (third_len, third) = integer(
            third_text,
            if year_first { 1 } else { 4 },
            if year_first { 2 } else { 4 },
        )?;
        let value = if year_first {
            date(first, second, third)
        } else if first <= 12 && second <= 12 && first != second && first != 0 && second != 0 {
            Value::Uncertain(FindingKind::Date, FindingStatus::Ambiguous)
        } else if first > 12 {
            date(third, second, first)
        } else {
            date(third, first, second)
        };
        return Some((text.len() - third_text.len() + third_len, value));
    }
    let month_text = rest.trim_start();
    if month_text.len() == rest.len() || first_digits > 2 {
        return None;
    }
    let (month_len, month) = month_name(month_text)?;
    let year_text = month_text[month_len..].trim_start();
    let (year_len, year) = integer(year_text, 4, 4)?;
    Some((
        text.len() - year_text.len() + year_len,
        date(year, month, first),
    ))
}

fn month_name(text: &str) -> Option<(usize, u32)> {
    const MONTHS: &[(&str, &str)] = &[
        ("January", "Jan"),
        ("February", "Feb"),
        ("March", "Mar"),
        ("April", "Apr"),
        ("May", "May"),
        ("June", "Jun"),
        ("July", "Jul"),
        ("August", "Aug"),
        ("September", "Sep"),
        ("October", "Oct"),
        ("November", "Nov"),
        ("December", "Dec"),
    ];
    if !text.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    for (index, (full, short)) in MONTHS.iter().enumerate() {
        if let Some((len, _)) = match_alias(text, &[full, short], false) {
            let dot = usize::from(len == short.len() && text[len..].starts_with('.'));
            return Some((len + dot, index as u32 + 1));
        }
    }
    None
}

fn date(year: u32, month: u32, day: u32) -> Value {
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    if year != 0 && (1..=max).contains(&day) {
        Value::Date(year, month, day)
    } else {
        Value::Uncertain(FindingKind::Date, FindingStatus::Unverifiable)
    }
}

#[derive(Clone, Copy)]
enum Meridiem {
    Am,
    Pm,
}

fn clock(text: &str) -> Option<(usize, Value)> {
    let (time_text, mut half) = if let Some(rest) = text.strip_prefix("上午") {
        (rest.trim_start(), Some(Meridiem::Am))
    } else if let Some(rest) = text.strip_prefix("下午") {
        (rest.trim_start(), Some(Meridiem::Pm))
    } else {
        (text, None)
    };
    let (hour_len, mut hour) = integer(time_text, 1, 2)?;
    let after_hour = &time_text[hour_len..];
    let mut second = 0;
    let (mut remaining, minute) = if let Some(rest) = after_hour.strip_prefix([':', '：']) {
        let (minute_len, minute) = integer(rest, 2, 2)?;
        let mut remaining = &rest[minute_len..];
        if let Some(second_text) = remaining.strip_prefix([':', '：']) {
            let (second_len, parsed) = integer(second_text, 2, 2)?;
            second = parsed;
            remaining = &second_text[second_len..];
        }
        (remaining, minute)
    } else {
        let marker = after_hour.chars().next()?;
        if !matches!(marker, '点' | '时') {
            return None;
        }
        let rest = after_hour[marker.len_utf8()..].trim_start();
        if let Some((minute_len, minute)) = integer(rest, 1, 2) {
            let remaining = rest[minute_len..].strip_prefix('分')?;
            if let Some((second_len, parsed)) = integer(remaining, 1, 2)
                && let Some(after_seconds) = remaining[second_len..].strip_prefix('秒')
            {
                second = parsed;
                (after_seconds, minute)
            } else {
                (remaining, minute)
            }
        } else if half.is_some() || marker == '时' {
            (&after_hour[marker.len_utf8()..], 0)
        } else {
            return None;
        }
    };

    let suffix = remaining.trim_start();
    if half.is_none()
        && let Some((used, spelling)) = match_alias(suffix, &["a.m.", "p.m.", "am", "pm"], false)
    {
        half = Some(if spelling.starts_with('a') {
            Meridiem::Am
        } else {
            Meridiem::Pm
        });
        remaining = &suffix[used..];
    }
    let valid_hour = if let Some(half) = half {
        let valid = (1..=12).contains(&hour);
        hour = hour % 12
            + match half {
                Meridiem::Am => 0,
                Meridiem::Pm => 12,
            };
        valid
    } else {
        hour < 24
    };
    let zone = remaining.trim_start();
    let known_zone = match_alias(
        zone,
        &[
            "UTC", "GMT", "PST", "PDT", "MST", "CST", "CDT", "EST", "EDT", "CET", "JST", "Z",
        ],
        false,
    );
    let value = if let Some((used, _)) = known_zone {
        remaining = &zone[used..];
        Value::Uncertain(FindingKind::Time, FindingStatus::Unverifiable)
    } else if valid_hour && minute < 60 && second < 60 {
        Value::Time(hour, minute, second)
    } else {
        Value::Uncertain(FindingKind::Time, FindingStatus::Unverifiable)
    };
    Some((text.len() - remaining.len(), value))
}

fn numeric(text: &str) -> Option<(usize, Value)> {
    if let Some((body, close)) = text
        .strip_prefix('(')
        .map(|body| (body, ')'))
        .or_else(|| text.strip_prefix('（').map(|body| (body, '）')))
    {
        let value_text = body.trim_start();
        let (used, value) = amount_or_number(value_text)?;
        let rest = value_text[used..].trim_start();
        if value.kind() == FindingKind::Money && rest.starts_with(close) {
            // 没有会计上下文时，括号可能代表负数，也可能只是附注。
            return Some((
                text.len() - rest.len() + close.len_utf8(),
                unknown(FindingKind::Money),
            ));
        }
        return None;
    }
    amount_or_number(text)
}

fn amount_or_number(text: &str) -> Option<(usize, Value)> {
    if let Some(rest) = text.strip_prefix("百分之") {
        let number = rest.trim_start();
        let (len, value) = decimal(number, None)?;
        return Some((
            text.len() - number.len() + len,
            value.map_or(unknown(FindingKind::Percentage), Value::Percentage),
        ));
    }
    let signed = sign(text);
    let (prefix_offset, prefix) = if let Some((sign_len, negative)) = signed {
        let after_sign = &text[sign_len..];
        if let Some((currency_len, currency)) = symbol(after_sign) {
            (sign_len + currency_len, Some((currency, Some(negative))))
        } else {
            (0, None)
        }
    } else if let Some((currency_len, currency)) = symbol(text) {
        (currency_len, Some((currency, None)))
    } else {
        (0, None)
    };
    let number_text = if prefix.is_some() {
        text[prefix_offset..].trim_start()
    } else {
        text
    };
    let (used, value) = decimal(number_text, prefix.and_then(|(_, sign)| sign))?;
    let number_len = text.len() - number_text.len() + used;
    let suffix = text[number_len..].trim_start();
    let suffix_prefix = text.len() - suffix.len();
    if let Some((currency, _)) = prefix {
        if let Some((currency_len, another)) = symbol(suffix) {
            let value =
                if currency == another || currency.may_be(another) || another.may_be(currency) {
                    let explicit = if currency.is_ambiguous() {
                        another
                    } else {
                        currency
                    };
                    value.map_or(unknown(FindingKind::Money), |number| {
                        Value::Money(number, explicit)
                    })
                } else {
                    unknown(FindingKind::Money)
                };
            return Some((suffix_prefix + currency_len, value));
        }
        return Some((
            number_len,
            value.map_or(unknown(FindingKind::Money), |number| {
                Value::Money(number, currency)
            }),
        ));
    }
    if let Some((currency_len, currency)) = symbol(suffix) {
        return Some((
            suffix_prefix + currency_len,
            value.map_or(unknown(FindingKind::Money), |number| {
                Value::Money(number, currency)
            }),
        ));
    }
    if let Some(marker) = suffix
        .chars()
        .next()
        .filter(|character| matches!(character, '%' | '％'))
    {
        return Some((
            suffix_prefix + marker.len_utf8(),
            value.map_or(unknown(FindingKind::Percentage), Value::Percentage),
        ));
    }
    if let Some((length, canonical_unit)) = unit(suffix) {
        return Some((
            suffix_prefix + length,
            value.map_or(unknown(FindingKind::Unit), |number| {
                Value::Unit(number, canonical_unit)
            }),
        ));
    }
    if let Some((suffix_len, suffix)) =
        match_alias(&text[number_len..], &["st", "nd", "rd", "th"], false)
    {
        let value = value
            .filter(|number| ordinal_suffix(number).is_some_and(|expected| expected == suffix))
            .map_or(unknown(FindingKind::Number), Value::Number);
        return Some((number_len + suffix_len, value));
    }
    if text[number_len..]
        .chars()
        .next()
        .is_some_and(ascii_identifier)
    {
        return None;
    }
    Some((
        number_len,
        value.map_or(unknown(FindingKind::Number), Value::Number),
    ))
}

fn ordinal_suffix(number: &str) -> Option<&'static str> {
    if number.starts_with('-') || number.contains('.') {
        return None;
    }
    let end = number.as_bytes();
    let suffix = if end.iter().rev().nth(1) == Some(&b'1') {
        "th"
    } else {
        match end.last() {
            Some(b'1') => "st",
            Some(b'2') => "nd",
            Some(b'3') => "rd",
            _ => "th",
        }
    };
    Some(suffix)
}

fn unknown(kind: FindingKind) -> Value {
    Value::Uncertain(kind, FindingStatus::Unverifiable)
}

fn decimal(text: &str, adjacent_sign: Option<bool>) -> Option<(usize, Option<String>)> {
    let (start, negative) = sign(text).unwrap_or((0, adjacent_sign.unwrap_or(false)));
    let mut valid = adjacent_sign.is_none() || start == 0;
    let (first_len, mut whole) = digits(&text[start..]);
    let mut cursor = start + first_len;
    let mut group_count = 0;
    while !whole.is_empty()
        && let Some((separator_len, part_len, part)) =
            separated_digits(&text[cursor..], &[',', '，'])
    {
        if group_count == 0 && whole.len() > 3 || part.len() != 3 {
            valid = false;
        }
        cursor += separator_len + part_len;
        whole.push_str(&part);
        group_count += 1;
    }
    let mut fraction = String::new();
    if let Some((separator_len, part_len, part)) = separated_digits(&text[cursor..], &['.', '．'])
    {
        cursor += separator_len + part_len;
        fraction = part;
    }
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if let Some((separator_len, part_len, _)) =
        separated_digits(&text[cursor..], &[',', '，', '/', '／'])
    {
        cursor += separator_len + part_len;
        valid = false;
    }
    if let Some(rest) = text[cursor..].strip_prefix(['e', 'E']) {
        let exp_sign = sign(rest).map_or(0, |(len, _)| len);
        let (len, _) = digits(&rest[exp_sign..]);
        if len != 0 {
            cursor += 1 + exp_sign + len;
            valid = false;
        }
    }
    let integer = whole.trim_start_matches('0');
    let fraction = fraction.trim_end_matches('0');
    let mut canonical = String::with_capacity(whole.len() + fraction.len() + 2);
    if negative && (!integer.is_empty() || !fraction.is_empty()) {
        canonical.push('-');
    }
    canonical.push_str(if integer.is_empty() { "0" } else { integer });
    if !fraction.is_empty() {
        canonical.push('.');
        canonical.push_str(fraction);
    }
    Some((cursor, valid.then_some(canonical)))
}

fn separated_digits(text: &str, separators: &[char]) -> Option<(usize, usize, String)> {
    let separator = text.chars().next()?;
    if !separators.contains(&separator) {
        return None;
    }
    let len = separator.len_utf8();
    let (digit_len, digits) = digits(&text[len..]);
    (digit_len != 0).then_some((len, digit_len, digits))
}

fn integer(text: &str, min: usize, max: usize) -> Option<(usize, u32)> {
    let mut value = 0;
    let mut count = 0;
    let mut length = 0;
    for byte in text.chars() {
        let Some(digit) = digit(byte) else { break };
        count += 1;
        if count > max {
            return None;
        }
        value = value * 10 + u32::from(digit);
        length += byte.len_utf8();
    }
    (count >= min).then_some((length, value))
}

fn digits(text: &str) -> (usize, String) {
    let mut length = 0;
    let mut out = String::new();
    for character in text.chars() {
        let Some(digit) = digit(character) else { break };
        length += character.len_utf8();
        out.push(char::from(b'0' + digit));
    }
    (length, out)
}

fn digit(character: char) -> Option<u8> {
    match character {
        '0'..='9' => Some(character as u8 - b'0'),
        '０'..='９' => Some((character as u32 - '０' as u32) as u8),
        _ => None,
    }
}

fn sign(text: &str) -> Option<(usize, bool)> {
    let character = text.chars().next()?;
    match character {
        '+' | '＋' => Some((character.len_utf8(), false)),
        '-' | '－' | '−' => Some((character.len_utf8(), true)),
        _ => None,
    }
}

fn match_alias<'a>(
    text: &str,
    aliases: &[&'a str],
    adjacent_digits: bool,
) -> Option<(usize, &'a str)> {
    aliases.iter().copied().find_map(|alias| {
        let slice = text.get(..alias.len())?;
        if !slice.eq_ignore_ascii_case(alias) {
            return None;
        }
        let boundary = text[alias.len()..]
            .chars()
            .next()
            .is_none_or(|next| !ascii_identifier(next) || adjacent_digits && next.is_ascii_digit());
        if alias.as_bytes().last().is_some_and(u8::is_ascii_alphabetic) && !boundary {
            return None;
        }
        Some((alias.len(), alias))
    })
}

fn ascii_identifier(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn symbol(text: &str) -> Option<(usize, Currency)> {
    use Currency::*;
    const CURRENCIES: &[(Currency, &[&str])] = &[
        (
            Cny,
            &[
                "Chinese yuan",
                "CN¥",
                "CN￥",
                "人民币",
                "CNY",
                "RMB",
                "yuan",
                "元",
            ],
        ),
        (Usd, &["US dollars", "US dollar", "美元", "USD", "US$"]),
        (
            Cad,
            &[
                "Canadian dollars",
                "Canadian dollar",
                "加元",
                "CAD",
                "CA$",
                "C$",
            ],
        ),
        (
            Aud,
            &[
                "Australian dollars",
                "Australian dollar",
                "澳元",
                "澳币",
                "AUD",
                "AU$",
                "A$",
            ],
        ),
        (
            Nzd,
            &[
                "New Zealand dollars",
                "New Zealand dollar",
                "新西兰元",
                "NZD",
                "NZ$",
            ],
        ),
        (
            Hkd,
            &[
                "Hong Kong dollars",
                "Hong Kong dollar",
                "港元",
                "港币",
                "HKD",
                "HK$",
            ],
        ),
        (
            Sgd,
            &[
                "Singapore dollars",
                "Singapore dollar",
                "新加坡元",
                "SGD",
                "S$",
            ],
        ),
        (
            Twd,
            &[
                "Taiwan dollars",
                "Taiwan dollar",
                "新台币",
                "台币",
                "TWD",
                "NT$",
            ],
        ),
        (Jpy, &["Japanese yen", "日元", "日圆", "JPY", "JP¥", "JP￥"]),
        (Eur, &["欧元", "EUR", "euros", "euro", "€"]),
        (
            Gbp,
            &["British pounds", "British pound", "英镑", "GBP", "£"],
        ),
        (DollarSymbol, &["$", "＄", "dollars", "dollar"]),
        (YenSymbol, &["¥", "￥", "yen"]),
    ];
    longest(text, CURRENCIES, true)
}

fn unit(text: &str) -> Option<(usize, Unit)> {
    use Unit::*;
    const UNITS: &[(Unit, &[&str])] = &[
        (
            Celsius,
            &[
                "degrees Celsius",
                "degree Celsius",
                "摄氏度",
                "摄氏",
                "°C",
                "℃",
            ],
        ),
        (
            Fahrenheit,
            &[
                "degrees Fahrenheit",
                "degree Fahrenheit",
                "华氏度",
                "华氏",
                "°F",
                "℉",
            ],
        ),
        (Degree, &["degrees", "degree", "度", "°"]),
        (
            Millimetre,
            &[
                "millimeters",
                "millimetres",
                "millimeter",
                "millimetre",
                "毫米",
                "mm",
            ],
        ),
        (
            Centimetre,
            &[
                "centimeters",
                "centimetres",
                "centimeter",
                "centimetre",
                "厘米",
                "cm",
            ],
        ),
        (
            Kilometre,
            &[
                "kilometers",
                "kilometres",
                "kilometer",
                "kilometre",
                "千米",
                "公里",
                "km",
            ],
        ),
        (Metre, &["meters", "metres", "meter", "metre", "米", "m"]),
        (Inch, &["inches", "inch", "英寸"]),
        (Foot, &["feet", "foot", "英尺", "ft"]),
        (Mile, &["miles", "mile", "英里", "mi"]),
        (Milligram, &["milligrams", "milligram", "毫克", "mg"]),
        (Kilogram, &["kilograms", "kilogram", "公斤", "千克", "kg"]),
        (Gram, &["grams", "gram", "克", "g"]),
        (
            Tonne,
            &["metric tons", "metric ton", "tonnes", "tonne", "吨"],
        ),
        (Pound, &["pounds", "pound", "磅", "lbs", "lb"]),
        (Ounce, &["ounces", "ounce", "盎司", "oz"]),
        (
            FluidOunce,
            &["fluid ounces", "fluid ounce", "液量盎司", "fl oz"],
        ),
        (
            Millilitre,
            &[
                "milliliters",
                "millilitres",
                "milliliter",
                "millilitre",
                "毫升",
                "ml",
            ],
        ),
        (
            Litre,
            &["liters", "litres", "liter", "litre", "公升", "升", "l"],
        ),
    ];
    let (len, value) = longest(text, UNITS, false)?;
    let raw = &text[..len];
    let ambiguous = matches!(raw, "M" | "ML" | "MG" | "MM" | "G")
        || raw.eq_ignore_ascii_case("pound")
        || raw.eq_ignore_ascii_case("pounds");
    Some((len, if ambiguous { Unit::Unclear } else { value }))
}

fn longest<T: Copy>(
    text: &str,
    table: &[(T, &[&str])],
    adjacent_digits: bool,
) -> Option<(usize, T)> {
    table
        .iter()
        .filter_map(|(value, spellings)| {
            match_alias(text, spellings, adjacent_digits).map(|(len, _)| (len, *value))
        })
        .max_by_key(|(len, _)| *len)
}
