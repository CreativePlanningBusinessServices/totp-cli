use serde_json::Value;

/// Every command yields one human line (possibly empty) and one JSON value;
/// `--json` picks which reaches stdout.
pub struct Outcome {
    pub human: String,
    pub json: Value,
}

impl Outcome {
    pub fn print(&self, as_json: bool) {
        if as_json {
            println!("{}", self.json);
        } else if !self.human.is_empty() {
            println!("{}", self.human);
        }
    }
}
