/// Copied from skim. See [ansi.rs](https://github.com/lotabout/skim/blob/master/src/ansi.rs)
// Parse ANSI style code
use std::default::Default;
use std::mem;

use beef::lean::Cow;
use ratatui::style::{Color, Modifier, Style};
use std::cmp::max;
use vte::{Params, Perform};

use crate::log_info;

/// An ANSI Parser, will parse one line at a time.
///
/// It will cache the latest style used, that means if an style affect multiple
/// lines, the parser will recognize it.
#[derive(Default)]
pub struct ANSIParser {
    partial_str: String,
    last_style: Style,

    stripped: String,
    stripped_char_count: usize,
    fragments: Vec<(Style, (u32, u32))>, // [char_index_start, char_index_end)
}

impl Perform for ANSIParser {
    fn print(&mut self, ch: char) {
        self.partial_str.push(ch);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            // \b to delete character back
            0x08 => {
                self.partial_str.pop();
            }
            // put back \0 \r \n \t
            0x00 | 0x0d | 0x0A | 0x09 => self.partial_str.push(byte as char),
            // ignore all others
            _ => log_info!("AnsiParser:execute ignored {:?}", byte),
        }
    }

    fn hook(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {
        log_info!("AnsiParser:hook ignored {:?}", params);
    }

    fn put(&mut self, byte: u8) {
        log_info!("AnsiParser:put ignored {:?}", byte);
    }

    fn unhook(&mut self) {
        log_info!("AnsiParser:unhook ignored");
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        log_info!("AnsiParser:osc ignored {:?}", params);
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters
        // Only care about graphic modes, ignore all others

        if action != 'm' {
            log_info!("ignore: params: {:?}, action : {:?}", params, action);
            return;
        }

        // \[[m => means reset
        let mut style = if params.is_empty() {
            Style::default()
        } else {
            self.last_style
        };

        let mut iter = params.iter();
        while let Some(code) = iter.next() {
            match code[0] {
                0 => style = Style::default(),
                1 => style.add_modifier |= Modifier::BOLD,
                2 => style.add_modifier |= !Modifier::BOLD,
                4 => style.add_modifier |= Modifier::UNDERLINED,
                5 => style.add_modifier |= Modifier::SLOW_BLINK,
                7 => style.add_modifier |= Modifier::REVERSED,
                num if (30..=37).contains(&num) => {
                    style.fg = Some(Color::Indexed((num - 30) as u8));
                }
                38 => match iter.next() {
                    Some(&[2]) => {
                        // ESC[ 38;2;<r>;<g>;<b> m Select RGB foreground color
                        let (r, g, b) = match (iter.next(), iter.next(), iter.next()) {
                            (Some(r), Some(g), Some(b)) => (r[0] as u8, g[0] as u8, b[0] as u8),
                            _ => {
                                log_info!("ignore CSI {:?} m", params);
                                continue;
                            }
                        };

                        style.fg = Some(Color::Rgb(r, g, b));
                    }
                    Some(&[5]) => {
                        // ESC[ 38;5;<n> m Select foreground color
                        let color = match iter.next() {
                            Some(color) => color[0] as u8,
                            None => {
                                log_info!("ignore CSI {:?} m", params);
                                continue;
                            }
                        };

                        style.fg = Some(Color::Indexed(color));
                    }
                    _ => {
                        log_info!("error on parsing CSI {:?} m", params);
                    }
                },
                39 => style.fg = Some(Color::Black),
                num if (40..=47).contains(&num) => {
                    style.bg = Some(Color::Indexed((num - 40) as u8));
                }
                48 => match iter.next() {
                    Some(&[2]) => {
                        // ESC[ 48;2;<r>;<g>;<b> m Select RGB background color
                        let (r, g, b) = match (iter.next(), iter.next(), iter.next()) {
                            (Some(r), Some(g), Some(b)) => (r[0] as u8, g[0] as u8, b[0] as u8),
                            _ => {
                                log_info!("ignore CSI {:?} m", params);
                                continue;
                            }
                        };

                        style.bg = Some(Color::Rgb(r, g, b));
                    }
                    Some(&[5]) => {
                        // ESC[ 48;5;<n> m Select background color
                        let color = match iter.next() {
                            Some(color) => color[0] as u8,
                            None => {
                                log_info!("ignore CSI {:?} m", params);
                                continue;
                            }
                        };

                        style.bg = Some(Color::Indexed(color));
                    }
                    _ => {
                        log_info!("ignore CSI {:?} m", params);
                    }
                },
                49 => style.bg = Some(Color::Black),
                _ => {
                    log_info!("ignore CSI {:?} m", params);
                }
            }
        }

        self.style_change(style);
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        // ESC characters are replaced with \[
        self.partial_str.push('"');
        self.partial_str.push('[');
    }
}

impl ANSIParser {
    /// save the partial_str into fragments with current style
    fn save_str(&mut self) {
        if self.partial_str.is_empty() {
            return;
        }

        let string = mem::take(&mut self.partial_str);
        let string_char_count = string.chars().count();
        self.fragments.push((
            self.last_style,
            (
                self.stripped_char_count as u32,
                (self.stripped_char_count + string_char_count) as u32,
            ),
        ));
        self.stripped_char_count += string_char_count;
        self.stripped.push_str(&string);
    }

    // accept a new style
    fn style_change(&mut self, new_style: Style) {
        if new_style == self.last_style {
            return;
        }

        self.save_str();
        self.last_style = new_style;
    }

    pub fn parse_ansi(&mut self, text: &str) -> AnsiString<'static> {
        let mut statemachine = vte::Parser::new();

        for byte in text.as_bytes() {
            statemachine.advance(self, *byte);
        }
        self.save_str();

        let stripped = mem::take(&mut self.stripped);
        self.stripped_char_count = 0;
        let fragments = mem::take(&mut self.fragments);
        AnsiString::new_string(stripped, fragments)
    }
}

/// A String that contains ANSI state (e.g. colors)
///
/// It is internally represented as Vec<(style, string)>
#[derive(Clone, Debug)]
pub struct AnsiString<'a> {
    stripped: Cow<'a, str>,
    // style: start, end
    fragments: Option<Vec<(Style, (u32, u32))>>,
}

impl<'a> AnsiString<'a> {
    pub fn new_empty() -> Self {
        Self {
            stripped: Cow::borrowed(""),
            fragments: None,
        }
    }

    fn new_raw_string(string: String) -> Self {
        Self {
            stripped: Cow::owned(string),
            fragments: None,
        }
    }

    fn new_raw_str(str_ref: &'a str) -> Self {
        Self {
            stripped: Cow::borrowed(str_ref),
            fragments: None,
        }
    }

    /// assume the fragments are ordered by (start, end) while end is exclusive
    pub fn new_str(stripped: &'a str, fragments: Vec<(Style, (u32, u32))>) -> Self {
        let fragments_empty =
            fragments.is_empty() || (fragments.len() == 1 && fragments[0].0 == Style::default());
        Self {
            stripped: Cow::borrowed(stripped),
            fragments: if fragments_empty {
                None
            } else {
                Some(fragments)
            },
        }
    }

    /// assume the fragments are ordered by (start, end) while end is exclusive
    pub fn new_string(stripped: String, fragments: Vec<(Style, (u32, u32))>) -> Self {
        let fragments_empty =
            fragments.is_empty() || (fragments.len() == 1 && fragments[0].0 == Style::default());
        Self {
            stripped: Cow::owned(stripped),
            fragments: if fragments_empty {
                None
            } else {
                Some(fragments)
            },
        }
    }

    pub fn parse(raw: &'a str) -> AnsiString<'static> {
        ANSIParser::default().parse_ansi(raw)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.stripped.is_empty()
    }

    #[inline]
    pub fn into_inner(self) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Owned(self.stripped.into_owned())
    }

    pub fn iter(&'a self) -> Box<dyn Iterator<Item = (char, Style)> + 'a> {
        if self.fragments.is_none() {
            return Box::new(self.stripped.chars().map(|c| (c, Style::default())));
        }

        Box::new(AnsiStringIterator::new(
            &self.stripped,
            self.fragments.as_ref().unwrap(),
        ))
    }

    pub fn has_styles(&self) -> bool {
        self.fragments.is_some()
    }

    #[inline]
    pub fn stripped(&self) -> &str {
        &self.stripped
    }

    pub fn override_styles(&mut self, styles: Vec<(Style, (u32, u32))>) {
        if styles.is_empty() {
            // pass
        } else if self.fragments.is_none() {
            self.fragments = Some(styles);
        } else {
            let current_fragments = self.fragments.take().expect("unreachable");
            let new_fragments = merge_fragments(&current_fragments, &styles);
            self.fragments.replace(new_fragments);
        }
    }
}

impl<'a> From<&'a str> for AnsiString<'a> {
    fn from(s: &'a str) -> AnsiString<'a> {
        AnsiString::new_raw_str(s)
    }
}

impl From<String> for AnsiString<'static> {
    fn from(s: String) -> Self {
        AnsiString::new_raw_string(s)
    }
}

// (text, indices, highlight styleibute) -> AnsiString
impl<'a> From<(&'a str, &'a [usize], Style)> for AnsiString<'a> {
    fn from((text, indices, style): (&'a str, &'a [usize], Style)) -> Self {
        let fragments = indices
            .iter()
            .map(|&idx| (style, (idx as u32, 1 + idx as u32)))
            .collect();
        AnsiString::new_str(text, fragments)
    }
}

/// An iterator over all the (char, style) characters.
pub struct AnsiStringIterator<'a> {
    fragments: &'a [(Style, (u32, u32))],
    fragment_idx: usize,
    chars_iter: std::iter::Enumerate<std::str::Chars<'a>>,
}

impl<'a> AnsiStringIterator<'a> {
    pub fn new(stripped: &'a str, fragments: &'a [(Style, (u32, u32))]) -> Self {
        Self {
            fragments,
            fragment_idx: 0,
            chars_iter: stripped.chars().enumerate(),
        }
    }
}

impl<'a> Iterator for AnsiStringIterator<'a> {
    type Item = (char, Style);

    fn next(&mut self) -> Option<Self::Item> {
        match self.chars_iter.next() {
            Some((char_idx, char)) => {
                // update fragment_idx
                loop {
                    if self.fragment_idx >= self.fragments.len() {
                        break;
                    }

                    let (_style, (_start, end)) = self.fragments[self.fragment_idx];
                    if char_idx < (end as usize) {
                        break;
                    } else {
                        self.fragment_idx += 1;
                    }
                }

                let (style, (start, end)) = if self.fragment_idx >= self.fragments.len() {
                    (Style::default(), (char_idx as u32, 1 + char_idx as u32))
                } else {
                    self.fragments[self.fragment_idx]
                };

                if (start as usize) <= char_idx && char_idx < (end as usize) {
                    Some((char, style))
                } else {
                    Some((char, Style::default()))
                }
            }
            None => None,
        }
    }
}

fn merge_fragments(
    old: &[(Style, (u32, u32))],
    new: &[(Style, (u32, u32))],
) -> Vec<(Style, (u32, u32))> {
    let mut ret = vec![];
    let mut i = 0;
    let mut j = 0;
    let mut os = 0;

    while i < old.len() && j < new.len() {
        let (oa, (o_start, oe)) = old[i];
        let (na, (ns, ne)) = new[j];
        os = max(os, o_start);

        if ns <= os && ne >= oe {
            //   [--old--]   | [--old--]   |   [--old--] | [--old--]
            // [----new----] | [---new---] | [---new---] | [--new--]
            i += 1; // skip old
        } else if ns <= os {
            //           [--old--] |         [--old--] |   [--old--] |   [---old---]
            // [--new--]           | [--new--]         | [--new--]   |   [--new--]
            ret.push((na, (ns, ne)));
            os = ne;
            j += 1;
        } else if ns >= oe {
            // [--old--]         | [--old--]
            //         [--new--] |           [--new--]
            ret.push((oa, (os, oe)));
            i += 1;
        } else {
            // [---old---] | [---old---] | [--old--]
            //  [--new--]  |   [--new--] |      [--new--]
            ret.push((oa, (os, ns)));
            os = ns;
        }
    }

    if i < old.len() {
        for &(oa, (s, e)) in old[i..].iter() {
            ret.push((oa, (max(os, s), e)))
        }
    }
    if j < new.len() {
        ret.extend_from_slice(&new[j..]);
    }

    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_iterator() {
        let input = "\x1B[48;2;5;10;15m\x1B[38;2;70;130;180mhi\x1B[0m";
        let ansistring = ANSIParser::default().parse_ansi(input);
        let mut it = ansistring.iter();
        let style = Style {
            fg: Some(Color::Rgb(70, 130, 180)),
            bg: Some(Color::Rgb(5, 10, 15)),
            ..Style::default()
        };

        assert_eq!(Some(('h', style)), it.next());
        assert_eq!(Some(('i', style)), it.next());
        assert_eq!(None, it.next());
        assert_eq!(ansistring.stripped(), "hi");
    }

    #[test]
    fn test_highlight_indices() {
        let text = "abc";
        let indices: Vec<usize> = vec![1];
        let style = Style {
            fg: Some(Color::Rgb(70, 130, 180)),
            bg: Some(Color::Rgb(5, 10, 15)),
            ..Style::default()
        };

        let ansistring = AnsiString::from((text, &indices as &[usize], style));
        let mut it = ansistring.iter();

        assert_eq!(Some(('a', Style::default())), it.next());
        assert_eq!(Some(('b', style)), it.next());
        assert_eq!(Some(('c', Style::default())), it.next());
        assert_eq!(None, it.next());
    }

    #[test]
    fn test_normal_string() {
        let input = "ab";
        let ansistring = ANSIParser::default().parse_ansi(input);

        assert!(!ansistring.has_styles());

        let mut it = ansistring.iter();
        assert_eq!(Some(('a', Style::default())), it.next());
        assert_eq!(Some(('b', Style::default())), it.next());
        assert_eq!(None, it.next());

        assert_eq!(ansistring.stripped(), "ab");
    }

    #[test]
    fn test_multiple_styleibutes() {
        let input = "\x1B[1;31mhi";
        let ansistring = ANSIParser::default().parse_ansi(input);
        let mut it = ansistring.iter();
        let style = Style {
            fg: Some(Color::Black),
            add_modifier: Modifier::BOLD,
            ..Style::default()
        };

        assert_eq!(Some(('h', style)), it.next());
        assert_eq!(Some(('i', style)), it.next());
        assert_eq!(None, it.next());
        assert_eq!(ansistring.stripped(), "hi");
    }

    #[test]
    fn test_reset() {
        let input = "\x1B[35mA\x1B[mB";
        let ansistring = ANSIParser::default().parse_ansi(input);
        assert_eq!(ansistring.fragments.as_ref().map(|x| x.len()).unwrap(), 2);
        assert_eq!(ansistring.stripped(), "AB");
    }

    #[test]
    fn test_multi_bytes() {
        let input = "中`\x1B[0m\x1B[1m\x1B[31mXYZ\x1B[0ms`";
        let ansistring = ANSIParser::default().parse_ansi(input);
        let mut it = ansistring.iter();
        let default_style = Style::default();
        let annotated = Style {
            fg: Some(Color::Black),
            add_modifier: Modifier::BOLD,
            ..default_style
        };

        assert_eq!(Some(('中', default_style)), it.next());
        assert_eq!(Some(('`', default_style)), it.next());
        assert_eq!(Some(('X', annotated)), it.next());
        assert_eq!(Some(('Y', annotated)), it.next());
        assert_eq!(Some(('Z', annotated)), it.next());
        assert_eq!(Some(('s', default_style)), it.next());
        assert_eq!(Some(('`', default_style)), it.next());
        assert_eq!(None, it.next());
    }

    #[test]
    fn test_merge_fragments() {
        let ao = Style::default();
        let an = Style::default().bg(Color::Blue);

        assert_eq!(
            merge_fragments(&[(ao, (0, 1)), (ao, (1, 2))], &[]),
            vec![(ao, (0, 1)), (ao, (1, 2))]
        );

        assert_eq!(
            merge_fragments(&[], &[(an, (0, 1)), (an, (1, 2))]),
            vec![(an, (0, 1)), (an, (1, 2))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 6)), (ao, (9, 10))],
                &[(an, (0, 1))]
            ),
            vec![(an, (0, 1)), (ao, (1, 3)), (ao, (5, 6)), (ao, (9, 10))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (0, 2))]
            ),
            vec![(an, (0, 2)), (ao, (2, 3)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (0, 3))]
            ),
            vec![(an, (0, 3)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (0, 6)), (an, (6, 7))]
            ),
            vec![(an, (0, 6)), (an, (6, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (1, 2))]
            ),
            vec![(an, (1, 2)), (ao, (2, 3)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (1, 3))]
            ),
            vec![(an, (1, 3)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (1, 4))]
            ),
            vec![(an, (1, 4)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (2, 3))]
            ),
            vec![(ao, (1, 2)), (an, (2, 3)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (2, 4))]
            ),
            vec![(ao, (1, 2)), (an, (2, 4)), (ao, (5, 7)), (ao, (9, 11))]
        );

        assert_eq!(
            merge_fragments(
                &[(ao, (1, 3)), (ao, (5, 7)), (ao, (9, 11))],
                &[(an, (2, 6))]
            ),
            vec![(ao, (1, 2)), (an, (2, 6)), (ao, (6, 7)), (ao, (9, 11))]
        );
    }

    #[test]
    fn test_multi_byte_359() {
        // https://github.com/lotabout/skim/issues/359
        let highlight = Style::default().add_modifier(Modifier::BOLD);
        let ansistring = AnsiString::new_str("ああa", vec![(highlight, (2, 3))]);
        let mut it = ansistring.iter();
        assert_eq!(Some(('あ', Style::default())), it.next());
        assert_eq!(Some(('あ', Style::default())), it.next());
        assert_eq!(Some(('a', highlight)), it.next());
        assert_eq!(None, it.next());
    }
}
