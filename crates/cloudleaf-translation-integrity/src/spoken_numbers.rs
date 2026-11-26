pub(crate) fn continues(character: char) -> bool {
    "零〇一二两兩三四五六七八九十百千万萬亿億点點半".contains(character)
}

pub(crate) fn cardinal(text: &str) -> Option<(usize, u8)> {
    let end = text
        .find(|character| !continues(character))
        .unwrap_or(text.len());
    if end != 0 {
        return chinese(&text[..end]).map(|number| (end, number));
    }
    let (width, value) = english_word(text, &SMALL)
        .or_else(|| english_word(text, &TENS).map(|(width, index)| (width, (index + 2) * 10)))?;
    let rest = &text[width..];
    if value >= 20
        && let Some(suffix) = rest.strip_prefix([' ', '-'])
        && let Some((used, units)) = english_word(suffix, &SMALL[..10])
        && units != 0
    {
        return Some((width + 1 + used, value + units));
    }
    Some((width, value))
}

const SMALL: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

const TENS: [&str; 8] = [
    "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn english_word(text: &str, vocabulary: &[&str]) -> Option<(usize, u8)> {
    vocabulary.iter().enumerate().find_map(|(index, word)| {
        let (head, tail) = text.split_at_checked(word.len())?;
        if !head.eq_ignore_ascii_case(word)
            || tail.starts_with(|character: char| {
                character.is_ascii_alphanumeric() || character == '_'
            })
        {
            return None;
        }
        Some((word.len(), index as u8))
    })
}

pub(crate) fn follows_scale(prefix: &str) -> bool {
    let mut words = prefix
        .trim_end()
        .rsplit(|character: char| !character.is_ascii_alphabetic());
    let last = words.next().unwrap_or_default();
    let scale = if last.eq_ignore_ascii_case("and") {
        words.find(|word| !word.is_empty()).unwrap_or_default()
    } else {
        last
    };
    ["hundred", "thousand", "million", "billion"]
        .iter()
        .any(|word| scale.eq_ignore_ascii_case(word))
}

fn chinese(text: &str) -> Option<u8> {
    let digit = |ch| {
        "零一二三四五六七八九"
            .chars()
            .position(|current| current == ch)
            .map(|index| index as u8)
            .or(match ch {
                '〇' => Some(0),
                '两' | '兩' => Some(2),
                _ => None,
            })
    };
    let mut chars = text.chars();
    let head = chars.next()?;
    let middle = chars.next();
    let end = chars.next();
    if chars.next().is_some() {
        return None;
    }
    match (head, middle, end) {
        ('十', None, None) => Some(10),
        ('十', Some(unit), None) => digit(unit).filter(|unit| *unit > 0).map(|unit| 10 + unit),
        (tens, Some('十'), end) if !matches!(tens, '两' | '兩') => {
            let tens = digit(tens).filter(|number| *number > 1)?;
            let units = match end {
                Some(unit) => digit(unit).filter(|unit| *unit > 0)?,
                None => 0,
            };
            Some(tens * 10 + units)
        }
        (_, None, None) => digit(head),
        _ => None,
    }
}

pub(crate) fn explicit_count(tail: &str) -> bool {
    let trimmed = tail.trim_start();
    if trimmed
        .starts_with(|character: char| "个個件只隻瓶箱包条條台臺人项項根张張片".contains(character))
    {
        return true;
    }
    let mut words = trimmed.split_ascii_whitespace();
    let nouns = [
        "item", "items", "piece", "pieces", "person", "people", "box", "boxes", "bottle",
        "bottles", "slot", "slots", "unit", "units", "copy", "copies",
    ];
    let is_count_noun = |word: &str| {
        let word = word.trim_end_matches([',', '.', '!', '?', ';']);
        nouns.iter().any(|noun| word.eq_ignore_ascii_case(noun))
    };
    trimmed.len() != tail.len()
        && words.next().is_some_and(|word| {
            is_count_noun(word)
                || (word.bytes().all(|value| value.is_ascii_alphabetic())
                    && !["and", "or", "to", "of", "the"]
                        .iter()
                        .any(|excluded| word.eq_ignore_ascii_case(excluded))
                    && words.next().is_some_and(is_count_noun))
        })
}
