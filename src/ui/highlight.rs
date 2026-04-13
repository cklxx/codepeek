/// Lightweight hand-rolled tokenizer for Rust/Python/JS.
/// Returns Vec<(text, Color)> for ratatui rendering.
use ratatui::style::Color;

// ─── Tokyo Night palette ─────────────────────────────────────────────────────
pub mod tn {
    use ratatui::style::Color;

    pub const BG: Color         = Color::Rgb(26, 27, 38);      // #1a1b26
    pub const BG_HL: Color      = Color::Rgb(40, 52, 87);      // #283457  selection
    pub const BG_DIM: Color     = Color::Rgb(31, 35, 53);      // #1f2335  alt bg
    pub const FG: Color         = Color::Rgb(192, 202, 245);   // #c0caf5  main text
    pub const FG_DIM: Color     = Color::Rgb(86, 95, 137);     // #565f89  muted
    pub const FG_DARK: Color    = Color::Rgb(59, 66, 97);      // #3b4261  very muted

    pub const KEYWORD: Color    = Color::Rgb(187, 154, 247);   // #bb9af7  purple
    pub const FUNC: Color       = Color::Rgb(122, 162, 247);   // #7aa2f7  blue
    pub const TYPE_: Color      = Color::Rgb(42, 195, 222);    // #2ac3de  cyan
    pub const STRING: Color     = Color::Rgb(158, 206, 106);   // #9ece6a  green
    pub const NUMBER: Color     = Color::Rgb(255, 158, 100);   // #ff9e64  orange
    pub const COMMENT: Color    = Color::Rgb(86, 95, 137);     // #565f89  muted blue-gray
    pub const OPERATOR: Color   = Color::Rgb(137, 221, 255);   // #89ddff  light blue
    pub const MACRO_: Color     = Color::Rgb(255, 117, 127);   // #ff757f  red-pink
    pub const LIFETIME: Color   = Color::Rgb(255, 199, 119);   // #ffc777  yellow
    pub const ATTR: Color       = Color::Rgb(65, 166, 181);    // #41a6b5  teal

    pub const CALLER_COLOR: Color  = Color::Rgb(158, 206, 106); // green  ← incoming
    pub const CALLEE_COLOR: Color  = Color::Rgb(255, 158, 100); // orange → outgoing
    pub const BORDER_FOCUS: Color  = Color::Rgb(122, 162, 247); // blue
    pub const BORDER_IDLE: Color   = Color::Rgb(59, 66, 97);    // very muted
    pub const SELECTED_FG: Color   = Color::Rgb(224, 175, 104); // #e0af68 gold
    pub const LINE_NUM: Color      = Color::Rgb(59, 66, 97);
    pub const BADGE_GREEN: Color   = Color::Rgb(115, 218, 202); // #73daca teal-green
}

// ─── Token kinds ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Span {
    pub text: String,
    pub color: Color,
}

impl Span {
    fn new(text: impl Into<String>, color: Color) -> Self {
        Span { text: text.into(), color }
    }
}

// ─── Per-language keyword tables ─────────────────────────────────────────────

const RUST_KW: &[&str] = &[
    "as","async","await","break","const","continue","crate","dyn","else","enum",
    "extern","false","fn","for","if","impl","in","let","loop","match","mod",
    "move","mut","pub","ref","return","self","Self","static","struct","super",
    "trait","true","type","unsafe","use","where","while",
];
const RUST_TYPES: &[&str] = &[
    "bool","u8","u16","u32","u64","u128","usize","i8","i16","i32","i64","i128",
    "isize","f32","f64","char","str","String","Vec","Option","Result","Box",
    "Arc","Rc","Cell","RefCell","HashMap","HashSet","BTreeMap","BTreeSet",
    "PathBuf","Path",
];
const PYTHON_KW: &[&str] = &[
    "False","None","True","and","as","assert","async","await","break","class",
    "continue","def","del","elif","else","except","finally","for","from",
    "global","if","import","in","is","lambda","nonlocal","not","or","pass",
    "raise","return","try","while","with","yield","self","cls",
];
const JS_KW: &[&str] = &[
    "async","await","break","case","catch","class","const","continue","debugger",
    "default","delete","do","else","export","extends","false","finally","for",
    "from","function","if","import","in","instanceof","let","new","null","of",
    "return","static","super","switch","this","throw","true","try","typeof",
    "undefined","var","void","while","with","yield",
];

// ─── Main highlight entry ─────────────────────────────────────────────────────

pub fn highlight_line(line: &str, lang: &crate::model::Language) -> Vec<Span> {
    use crate::model::Language::*;
    match lang {
        Rust => tokenize(line, RUST_KW, RUST_TYPES, "//", true),
        Python => tokenize(line, PYTHON_KW, &[], "#", false),
        JavaScript | TypeScript => tokenize(line, JS_KW, &[], "//", false),
        Unknown => vec![Span::new(line, tn::FG)],
    }
}

fn tokenize(
    line: &str,
    keywords: &[&str],
    types: &[&str],
    line_comment: &str,
    rust_extras: bool,
) -> Vec<Span> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        // Line comment
        let rest: String = chars[i..].iter().collect();
        if rest.starts_with(line_comment) {
            spans.push(Span::new(&rest, tn::COMMENT));
            break;
        }

        // Block comment start (Rust /* */)
        if rust_extras && rest.starts_with("/*") {
            spans.push(Span::new(&rest, tn::COMMENT));
            break;
        }

        // Doc attr #[
        if rust_extras && chars[i] == '#' {
            let attr_end = chars[i..].iter().position(|&c| c == ']').map(|p| i + p + 1).unwrap_or(n);
            let text: String = chars[i..attr_end].iter().collect();
            spans.push(Span::new(text, tn::ATTR));
            i = attr_end;
            continue;
        }

        // Lifetime 'a
        if rust_extras && chars[i] == '\'' && i + 1 < n && chars[i+1].is_alphabetic() {
            let end = chars[i+1..].iter().position(|c| !c.is_alphanumeric() && *c != '_')
                .map(|p| i + 1 + p).unwrap_or(n);
            let text: String = chars[i..end].iter().collect();
            spans.push(Span::new(text, tn::LIFETIME));
            i = end;
            continue;
        }

        // String literal " or '
        if chars[i] == '"' || (chars[i] == '\'' && !rust_extras) {
            let quote = chars[i];
            let mut j = i + 1;
            while j < n {
                if chars[j] == '\\' { j += 2; continue; }
                if chars[j] == quote { j += 1; break; }
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            spans.push(Span::new(text, tn::STRING));
            i = j;
            continue;
        }

        // Number
        if chars[i].is_ascii_digit() || (chars[i] == '-' && i + 1 < n && chars[i+1].is_ascii_digit()) {
            let mut j = i;
            if chars[j] == '-' { j += 1; }
            while j < n && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_') {
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            spans.push(Span::new(text, tn::NUMBER));
            i = j;
            continue;
        }

        // Identifier / keyword
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut j = i;
            while j < n && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();

            // Macro call: word!
            let color = if rust_extras && j < n && chars[j] == '!' {
                j += 1; // include the !
                tn::MACRO_
            } else if keywords.contains(&word.as_str()) {
                tn::KEYWORD
            } else if types.contains(&word.as_str()) {
                tn::TYPE_
            } else if j < n && chars[j] == '(' {
                tn::FUNC
            } else if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                tn::TYPE_
            } else {
                tn::FG
            };

            let text: String = chars[i..j].iter().collect();
            spans.push(Span::new(text, color));
            i = j;
            continue;
        }

        // Operators and punctuation
        let op_chars = "=<>!&|+-*/^%~?:;,@";
        if op_chars.contains(chars[i]) {
            spans.push(Span::new(chars[i].to_string(), tn::OPERATOR));
            i += 1;
            continue;
        }

        // Everything else (brackets, whitespace)
        spans.push(Span::new(chars[i].to_string(), tn::FG));
        i += 1;
    }

    spans
}
