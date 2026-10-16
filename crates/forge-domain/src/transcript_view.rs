/// How much of a session the chat shows (Claude Code desktop: 트랜스크립트 보기).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptView {
    /// Answers, and one collapsed line per tool call.
    #[default]
    Normal,
    /// Normal, plus what the model thought before it answered.
    Thinking,
    /// Thinking, plus every tool call opened with its input and result.
    Detailed,
}

impl TranscriptView {
    pub const ALL: [TranscriptView; 3] = [TranscriptView::Normal, TranscriptView::Thinking, TranscriptView::Detailed];

    /// The view after this one, going back to Normal after Detailed.
    pub fn next(self) -> TranscriptView {
        match self {
            TranscriptView::Normal => TranscriptView::Thinking,
            TranscriptView::Thinking => TranscriptView::Detailed,
            TranscriptView::Detailed => TranscriptView::Normal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TranscriptView::Normal => crate::msg!("Normal"),
            TranscriptView::Thinking => crate::msg!("Thinking"),
            TranscriptView::Detailed => crate::msg!("Detailed"),
        }
    }

    pub fn shows_thinking(self) -> bool {
        self != TranscriptView::Normal
    }

    pub fn expands_tool_calls(self) -> bool {
        self == TranscriptView::Detailed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_goes_round_the_three_views() {
        assert_eq!(TranscriptView::Normal.next(), TranscriptView::Thinking);
        assert_eq!(TranscriptView::Thinking.next(), TranscriptView::Detailed);
        assert_eq!(TranscriptView::Detailed.next(), TranscriptView::Normal);
    }

    #[test]
    fn normal_is_the_default() {
        assert_eq!(TranscriptView::default(), TranscriptView::Normal);
    }

    #[test]
    fn only_thinking_and_detailed_show_thinking() {
        assert!(!TranscriptView::Normal.shows_thinking());
        assert!(TranscriptView::Thinking.shows_thinking());
        assert!(TranscriptView::Detailed.shows_thinking());
    }

    #[test]
    fn only_detailed_opens_tool_calls() {
        assert!(!TranscriptView::Normal.expands_tool_calls());
        assert!(!TranscriptView::Thinking.expands_tool_calls());
        assert!(TranscriptView::Detailed.expands_tool_calls());
    }

    #[test]
    fn labels_are_distinct() {
        let labels: Vec<_> = TranscriptView::ALL.iter().map(|v| v.label()).collect();
        assert_eq!(labels, vec!["Normal", "Thinking", "Detailed"]);
    }
}
