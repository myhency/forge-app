use std::collections::BTreeMap;

use serde_json::Value;

/// The tool whose permission request is a question to the person, not a permission.
pub const QUESTION_TOOL: &str = "AskUserQuestion";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub header: String,
    pub text: String,
    pub options: Vec<QuestionOption>,
    pub multi: bool,
}

fn string_field(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The questions in an `AskUserQuestion` input. Anything missing or malformed is skipped: a
/// question without text is dropped, a missing header or description is empty.
pub fn questions_of(input: &Value) -> Vec<Question> {
    let Some(list) = input.get("questions").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|question| {
            let text = string_field(question, "question");
            if text.is_empty() {
                return None;
            }
            let options = question
                .get("options")
                .and_then(Value::as_array)
                .map(|options| {
                    options
                        .iter()
                        .map(|option| QuestionOption { label: string_field(option, "label"), description: string_field(option, "description") })
                        .collect()
                })
                .unwrap_or_default();
            Some(Question {
                header: string_field(question, "header"),
                text,
                options,
                multi: question.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// What the person has chosen for one question: option indexes (kept in option order) and the
/// text typed under "Other".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picks {
    pub chosen: Vec<usize>,
    pub other: String,
}

impl Picks {
    /// A single-select question holds one pick (and no "Other" text); a multi-select one toggles.
    pub fn toggle(&mut self, multi: bool, index: usize) {
        if multi {
            match self.chosen.iter().position(|&picked| picked == index) {
                Some(position) => {
                    self.chosen.remove(position);
                }
                None => {
                    self.chosen.push(index);
                    self.chosen.sort_unstable();
                }
            }
        } else {
            self.chosen = vec![index];
            self.other.clear();
        }
    }

    /// Typing under "Other" on a single-select question replaces the pick.
    pub fn set_other(&mut self, multi: bool, text: String) {
        if !multi && !text.trim().is_empty() {
            self.chosen.clear();
        }
        self.other = text;
    }

    pub fn ready(&self) -> bool {
        !self.chosen.is_empty() || !self.other.trim().is_empty()
    }
}

/// The answers the sidecar takes: each question's text to the chosen labels joined by a bare comma
/// (the real app does the same), then the "Other" text. A question with nothing picked is left out.
pub fn answers_map(questions: &[Question], picks: &[Picks]) -> BTreeMap<String, String> {
    let mut answers = BTreeMap::new();
    for (question, picked) in questions.iter().zip(picks) {
        let mut parts: Vec<String> = picked
            .chosen
            .iter()
            .filter_map(|&index| question.options.get(index))
            .map(|option| option.label.clone())
            .collect();
        if !picked.other.trim().is_empty() {
            parts.push(picked.other.trim().to_string());
        }
        if !parts.is_empty() {
            answers.insert(question.text.clone(), parts.join(","));
        }
    }
    answers
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerPair {
    pub question: String,
    pub answer: String,
}

const ANSWERED_PREFIX: &str = "Your questions have been answered: ";
const ANSWERED_SUFFIX: &str = "\". You can now continue";

/// The question and answer pairs of a finished `AskUserQuestion` call, read from its result text
/// (`Your questions have been answered: "Q"="A", "Q2"="A2". You can now continue ...`), matched
/// against the question texts in the call's input. A result that is not an answer gives none.
/// Several labels (joined by a bare comma in the text) are shown with `, `.
pub fn answer_pairs(input: &Value, output: &str) -> Vec<AnswerPair> {
    let Some(body) = output.strip_prefix(ANSWERED_PREFIX) else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    let mut cursor = 0;
    for question in questions_of(input) {
        let marker = format!("\"{}\"=\"", question.text);
        let Some(found) = body[cursor..].find(&marker) else { continue };
        let start = cursor + found + marker.len();
        let rest = &body[start..];
        // An answer ends at the quote before `, "<next question>"=` or before `. You can now continue`.
        let end = ["\", \"", ANSWERED_SUFFIX]
            .iter()
            .filter_map(|stop| rest.find(stop))
            .min()
            .unwrap_or(rest.len());
        let answer = rest[..end].replace(',', ", ");
        pairs.push(AnswerPair { question: question.text, answer });
        cursor = start + end;
    }
    pairs
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn input() -> serde_json::Value {
        json!({"questions": [
            {"question": "Colours?", "header": "Colours", "multiSelect": true,
             "options": [{"label": "Blue", "description": "sky"}, {"label": "Green", "description": "leaf"}, {"label": "Red", "description": "fire"}]},
            {"question": "Season?", "header": "Season", "multiSelect": false,
             "options": [{"label": "Spring", "description": "a"}, {"label": "Summer", "description": "b"}]}
        ]})
    }

    #[test]
    fn questions_are_read_from_the_tool_input() {
        let questions = questions_of(&input());
        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].header, "Colours");
        assert_eq!(questions[0].text, "Colours?");
        assert!(questions[0].multi);
        assert!(!questions[1].multi);
        assert_eq!(questions[0].options[2], QuestionOption { label: "Red".to_string(), description: "fire".to_string() });
    }

    #[test]
    fn malformed_input_gives_no_questions_and_never_panics() {
        for bad in [json!(null), json!({}), json!({"questions": 5}), json!({"questions": [1, "x", null]}), json!("text")] {
            assert!(questions_of(&bad).is_empty(), "{bad}");
        }
    }

    #[test]
    fn a_question_with_no_text_is_dropped_and_missing_fields_default() {
        let questions = questions_of(&json!({"questions": [{"header": "h"}, {"question": "Only text?"}]}));
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].text, "Only text?");
        assert_eq!(questions[0].header, "");
        assert!(questions[0].options.is_empty());
        assert!(!questions[0].multi);
    }

    #[test]
    fn a_single_select_pick_replaces_the_pick_and_clears_other() {
        let mut picks = Picks::default();
        picks.set_other(false, "my own".to_string());
        picks.toggle(false, 1);
        assert_eq!(picks.chosen, vec![1]);
        assert_eq!(picks.other, "");
        picks.toggle(false, 0);
        assert_eq!(picks.chosen, vec![0]);
    }

    #[test]
    fn typing_other_on_a_single_select_clears_the_pick() {
        let mut picks = Picks::default();
        picks.toggle(false, 1);
        picks.set_other(false, "mine".to_string());
        assert!(picks.chosen.is_empty());
        assert_eq!(picks.other, "mine");
    }

    #[test]
    fn a_multi_select_toggles_and_keeps_other() {
        let mut picks = Picks::default();
        picks.toggle(true, 2);
        picks.toggle(true, 0);
        picks.set_other(true, "also this".to_string());
        assert_eq!(picks.chosen, vec![0, 2], "kept in option order");
        picks.toggle(true, 2);
        assert_eq!(picks.chosen, vec![0]);
        assert_eq!(picks.other, "also this");
    }

    #[test]
    fn ready_needs_a_pick_or_some_typed_text() {
        let mut picks = Picks::default();
        assert!(!picks.ready());
        picks.set_other(true, "   ".to_string());
        assert!(!picks.ready());
        picks.set_other(true, "x".to_string());
        assert!(picks.ready());
        let mut chosen = Picks::default();
        chosen.toggle(false, 0);
        assert!(chosen.ready());
    }

    #[test]
    fn answers_join_labels_with_a_bare_comma_and_append_other() {
        let questions = questions_of(&input());
        let mut first = Picks::default();
        first.toggle(true, 0);
        first.toggle(true, 1);
        first.set_other(true, "Pink".to_string());
        let mut second = Picks::default();
        second.toggle(false, 1);
        let answers = answers_map(&questions, &[first, second]);
        assert_eq!(answers["Colours?"], "Blue,Green,Pink");
        assert_eq!(answers["Season?"], "Summer");
    }

    #[test]
    fn a_question_with_no_picks_is_left_out_of_the_answers() {
        let questions = questions_of(&input());
        let mut first = Picks::default();
        first.toggle(true, 0);
        let answers = answers_map(&questions, &[first]);
        assert_eq!(answers.len(), 1);
    }

    #[test]
    fn answer_pairs_are_read_from_the_result_text() {
        let output = "Your questions have been answered: \"Colours?\"=\"Blue,Green\", \"Season?\"=\"Summer\". You can now continue with these answers in mind.";
        let pairs = answer_pairs(&input(), output);
        assert_eq!(
            pairs,
            vec![
                AnswerPair { question: "Colours?".to_string(), answer: "Blue, Green".to_string() },
                AnswerPair { question: "Season?".to_string(), answer: "Summer".to_string() },
            ]
        );
    }

    #[test]
    fn answer_pairs_survive_quotes_commas_and_korean_text() {
        let input = json!({"questions": [
            {"question": "좋아하는 색을 모두 골라 주세요.", "options": []},
            {"question": "Say \"hi\", ok?", "options": []}
        ]});
        let output = "Your questions have been answered: \"좋아하는 색을 모두 골라 주세요.\"=\"파랑,초록\", \"Say \"hi\", ok?\"=\"yes, \"sure\"\". You can now continue with these answers in mind.";
        let pairs = answer_pairs(&input, output);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].answer, "파랑, 초록");
        assert_eq!(pairs[1].question, "Say \"hi\", ok?");
        assert_eq!(pairs[1].answer, "yes,  \"sure\"");
    }

    #[test]
    fn a_result_that_is_not_an_answer_gives_no_pairs() {
        assert!(answer_pairs(&input(), "The user skipped the question.").is_empty());
        assert!(answer_pairs(&input(), "").is_empty());
        assert!(answer_pairs(&json!({}), "Your questions have been answered: \"a\"=\"b\".").is_empty());
    }
}
