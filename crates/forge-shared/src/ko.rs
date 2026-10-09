//! The Korean text for each English sentence, sorted by the English text (byte order).
//! Keep it in step with the `tr!` and `msg!` calls: a test in `i18n.rs` checks both ways.

pub static KO: &[(&str, &str)] = &[
    ("Account menu", "계정 메뉴"),
    ("Get API key", "API 키 받기"),
    ("Get help", "도움 받기"),
    ("Keyboard shortcuts", "단축키"),
    ("Language", "언어"),
    ("Learn more", "더 알아보기"),
    ("Privacy policy", "개인정보 처리방침"),
    ("Settings", "설정"),
    ("Usage", "사용량"),
    ("Usage policy", "이용 정책"),
];
