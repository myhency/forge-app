use forge_domain::{Message, MessageRole};

/// How much of a session the chat shows (Claude Code desktop: 트랜스크립트 보기).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

    pub fn label(self) -> &'static str {
        match self {
            TranscriptView::Normal => "Normal",
            TranscriptView::Thinking => "Thinking",
            TranscriptView::Detailed => "Detailed",
        }
    }

    pub fn shows_thinking(self) -> bool {
        self != TranscriptView::Normal
    }

    pub fn expands_tool_calls(self) -> bool {
        self == TranscriptView::Detailed
    }

    /// Whether the message has anything to show in this view. A message of only thinking has
    /// nothing in Normal, and must not leave an empty bubble.
    pub fn shows(self, message: &Message) -> bool {
        match message.role {
            MessageRole::User => true,
            MessageRole::Assistant => {
                !message.content.is_empty()
                    || !message.tool_calls.is_empty()
                    || (self.shows_thinking() && !message.thinking.is_empty())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::ToolCall;
    use serde_json::json;

    fn assistant(content: &str, thinking: &str, tools: usize) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.content = content.to_string();
        message.thinking = thinking.to_string();
        for i in 0..tools {
            message.tool_calls.push(ToolCall { id: format!("t{i}"), name: "Read".to_string(), input: json!({}), result: None });
        }
        message
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
    fn a_message_of_only_thinking_is_hidden_in_normal_and_shown_otherwise() {
        let thought = assistant("", "hmm", 0);
        assert!(!TranscriptView::Normal.shows(&thought));
        assert!(TranscriptView::Thinking.shows(&thought));
        assert!(TranscriptView::Detailed.shows(&thought));
    }

    #[test]
    fn an_answer_or_a_tool_call_is_shown_in_every_view() {
        for view in TranscriptView::ALL {
            assert!(view.shows(&assistant("hi", "", 0)), "{view:?} answer");
            assert!(view.shows(&assistant("", "", 1)), "{view:?} tool call");
        }
    }

    #[test]
    fn an_empty_assistant_message_is_never_shown() {
        for view in TranscriptView::ALL {
            assert!(!view.shows(&assistant("", "", 0)), "{view:?}");
        }
    }

    #[test]
    fn a_user_message_is_always_shown() {
        for view in TranscriptView::ALL {
            assert!(view.shows(&Message::new(MessageRole::User)), "{view:?}");
        }
    }

    #[test]
    fn labels_are_distinct() {
        let labels: Vec<_> = TranscriptView::ALL.iter().map(|v| v.label()).collect();
        assert_eq!(labels, vec!["Normal", "Thinking", "Detailed"]);
    }
}
