/// The system's own dictation, started for the field that has the keyboard focus: the microphone
/// button of the message box. The desktop app streams the voice to Anthropic; Forge hands it to the
/// operating system (macOS Dictation, Windows voice typing) instead.
pub trait DictationPort: Send + Sync {
    /// Whether this system has dictation Forge can start (the button is shown only then).
    fn available(&self) -> bool;
    /// Starts it; an error says why it could not, for the person.
    fn start(&self) -> Result<(), String>;
}
