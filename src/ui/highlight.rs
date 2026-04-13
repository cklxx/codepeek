/// Lightweight tokenizer — color philosophy:
/// Keywords are PERIPHERAL (low saturation, muted): `pub fn if for` processed without fixation.
/// Identifiers/names are PRIMARY (high saturation, bright): where the eye lands.
/// Types are SECONDARY (medium saturation cyan).
/// This matches eye-tracking research: programmers fixate on identifiers, not syntax.
use ratatui::style::Color;

// ─── Color palette (Tokyo Night, attention-calibrated) ────────────────────────
pub mod tn {
    use ratatui::style::Color;

    // Backgrounds
    pub const BG: Color       = Color::Rgb(26, 27, 38);    // #1a1b26
    pub const BG_HL: Color    = Color::Rgb(36, 40, 67);    // #242843 — gentle selection
    pub const BG_DIM: Color   = Color::Rgb(31, 35, 53);    // #1f2335 — code block bg
    pub const BG_SEL: Color   = Color::Rgb(45, 50, 80);    // brighter selection for fn list

    // Foregrounds — 4 levels of visual weight
    pub const FG: Color       = Color::Rgb(169, 177, 214); // #a9b1d6 — body text
    pub const FG_MED: Color   = Color::Rgb(122, 130, 170); // #7a82aa — secondary text
    pub const FG_DIM: Color   = Color::Rgb(73, 82, 120);   // #495278 — muted/peripheral
    pub const FG_DARK: Color  = Color::Rgb(47, 53, 83);    // #2f3553 — barely visible

    // Semantic — primary attention targets (high saturation)
    pub const NAME: Color     = Color::Rgb(122, 162, 247); // #7aa2f7 — fn name (PRIMARY)
    pub const TYPE_: Color    = Color::Rgb(42, 195, 222);  // #2ac3de — types (SECONDARY)
    pub const STRING: Color   = Color::Rgb(158, 206, 106); // #9ece6a — strings
    pub const NUMBER: Color   = Color::Rgb(255, 158, 100); // #ff9e64 — numbers
    pub const MACRO_: Color   = Color::Rgb(224, 175, 104); // #e0af68 — macros (warm)

    // Low-saturation — peripheral processing (keywords, operators)
    pub const KW_CTRL: Color  = Color::Rgb(65, 72, 104);   // #414868 — if/for/while/let
    pub const KW_DEF: Color   = Color::Rgb(86, 95, 137);   // #565f89 — fn/pub/struct/impl
    pub const COMMENT: Color  = Color::Rgb(59, 66, 97);    // #3b4261 — comments
    pub const OPERATOR: Color = Color::Rgb(86, 95, 137);   // #565f89 — = + - * operators
    pub const LIFETIME: Color = Color::Rgb(180, 249, 248); // #b4f9f8 — 'a lifetimes
    pub const ATTR: Color     = Color::Rgb(65, 102, 114);  // #416672 — #[derive...]

    // UI chrome
    pub const BORDER_FOCUS: Color = Color::Rgb(122, 162, 247); // blue — focused panel
    pub const BORDER_IDLE: Color  = Color::Rgb(47, 53, 83);    // very dim
    pub const SELECTED_FG: Color  = Color::Rgb(224, 175, 104); // #e0af68 gold — selected row
    pub const CALLER_COLOR: Color = Color::Rgb(115, 218, 202); // #73daca teal — ← callers
    pub const CALLEE_COLOR: Color = Color::Rgb(255, 158, 100); // #ff9e64 orange — → callees
    pub const LINENUM: Color      = Color::Rgb(41, 46, 66);    // barely visible gutter
    pub const LINENUM_ACTIVE: Color = Color::Rgb(73, 82, 120); // slightly brighter for fn start
}

// ─── Span ─────────────────────────────────────────────────────────────────────

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

// ─── Keyword tables ───────────────────────────────────────────────────────────

// Control flow / binding — processed peripherally, very muted
const RUST_KW_CTRL: &[&str] = &[
    "as","break","continue","else","for","if","in","let","loop","match",
    "move","return","use","where","while","false","true",
];
// Definition keywords — slightly more visible (structural but important)
const RUST_KW_DEF: &[&str] = &[
    "async","await","const","crate","dyn","enum","extern","fn","impl",
    "mod","mut","pub","ref","self","Self","static","struct","super","trait","type","unsafe",
];
const RUST_TYPES: &[&str] = &[
    "bool","u8","u16","u32","u64","u128","usize","i8","i16","i32","i64","i128",
    "isize","f32","f64","char","str","String","Vec","Option","Result","Box",
    "Arc","Rc","Cell","RefCell","HashMap","HashSet","BTreeMap","BTreeSet",
    "PathBuf","Path","Cow","Pin","Future","Stream",
];
const PYTHON_KW_CTRL: &[&str] = &[
    "and","as","assert","break","continue","del","elif","else","except",
    "finally","for","from","global","if","import","in","is","lambda",
    "nonlocal","not","or","pass","raise","return","try","while","with","yield",
    "False","None","True",
];
const PYTHON_KW_DEF: &[&str] = &["async","await","class","def","self","cls"];
const JS_KW_CTRL: &[&str] = &[
    "break","case","catch","continue","debugger","default","delete","do","else",
    "finally","for","if","in","instanceof","new","return","switch","throw","try",
    "typeof","void","while","with","yield","false","null","true","undefined","of","from",
];
const JS_KW_DEF: &[&str] = &[
    "async","await","class","const","export","extends","function","import",
    "let","static","super","this","var",
];

// ─── Entry point ─────────────────────────────────────────────────────────────

pub fn highlight_line(line: &str, lang: &crate::model::Language) -> Vec<Span> {
    use crate::model::Language::*;
    // Catch any panic so a bad line never crashes the TUI
    std::panic::catch_unwind(|| match lang {
        Rust => tokenize(line, RUST_KW_CTRL, RUST_KW_DEF, RUST_TYPES, "//", true),
        Python => tokenize(line, PYTHON_KW_CTRL, PYTHON_KW_DEF, &[], "#", false),
        JavaScript | TypeScript => tokenize(line, JS_KW_CTRL, JS_KW_DEF, &[], "//", false),
        Unknown => vec![Span::new(line, tn::FG)],
    })
    .unwrap_or_else(|_| vec![Span::new(line, tn::FG)])
}

// ─── Tokenizer ────────────────────────────────────────────────────────────────

fn tokenize(
    line: &str,
    kw_ctrl: &[&str],
    kw_def: &[&str],
    types: &[&str],
    line_comment: &str,
    rust_extras: bool,
) -> Vec<Span> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let rest: String = chars[i..].iter().collect();

        // Line comment
        if rest.starts_with(line_comment) {
            spans.push(Span::new(rest, tn::COMMENT));
            break;
        }
        // Block comment
        if rust_extras && rest.starts_with("/*") {
            spans.push(Span::new(rest, tn::COMMENT));
            break;
        }

        // Attribute #[...]
        if rust_extras && chars[i] == '#' {
            let end = chars[i..].iter()
                .position(|&c| c == ']')
                .map(|p| (i + p + 1).min(n))
                .unwrap_or(n);
            let text: String = chars[i..end].iter().collect();
            spans.push(Span::new(text, tn::ATTR));
            i = end;
            continue;
        }

        // Lifetime 'a
        if rust_extras && chars[i] == '\'' && i + 1 < n && chars[i + 1].is_alphabetic() {
            let end = chars[i + 1..]
                .iter()
                .position(|c| !c.is_alphanumeric() && *c != '_')
                .map(|p| i + 1 + p)
                .unwrap_or(n);
            let end = end.min(n);
            let text: String = chars[i..end].iter().collect();
            spans.push(Span::new(text, tn::LIFETIME));
            i = end;
            continue;
        }

        // String literal
        if chars[i] == '"' || (chars[i] == '\'' && !rust_extras) {
            let quote = chars[i];
            let mut j = i + 1;
            while j < n {
                if chars[j] == '\\' {
                    j = (j + 2).min(n); // ← fix: cap at n to prevent OOB
                    continue;
                }
                if chars[j] == quote {
                    j += 1;
                    break;
                }
                j += 1;
            }
            let j = j.min(n);
            let text: String = chars[i..j].iter().collect();
            spans.push(Span::new(text, tn::STRING));
            i = j;
            continue;
        }

        // Number
        if chars[i].is_ascii_digit()
            || (chars[i] == '-' && i + 1 < n && chars[i + 1].is_ascii_digit())
        {
            let mut j = i;
            if chars[j] == '-' { j += 1; }
            while j < n
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_')
            {
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

            // Macro: word!
            let (color, j) = if rust_extras && j < n && chars[j] == '!' {
                (tn::MACRO_, j + 1)
            } else if kw_ctrl.contains(&word.as_str()) {
                (tn::KW_CTRL, j)
            } else if kw_def.contains(&word.as_str()) {
                (tn::KW_DEF, j)
            } else if types.contains(&word.as_str()) {
                (tn::TYPE_, j)
            } else if j < n && chars[j] == '(' {
                (tn::NAME, j) // call site
            } else if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                (tn::TYPE_, j)
            } else {
                (tn::FG, j)
            };

            let j = j.min(n);
            let text: String = chars[i..j].iter().collect();
            spans.push(Span::new(text, color));
            i = j;
            continue;
        }

        // Operators
        if "=<>!&|+-*/^%~?:;,@".contains(chars[i]) {
            spans.push(Span::new(chars[i].to_string(), tn::OPERATOR));
            i += 1;
            continue;
        }

        // Everything else
        spans.push(Span::new(chars[i].to_string(), tn::FG));
        i += 1;
    }

    spans
}
