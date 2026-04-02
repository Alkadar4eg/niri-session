//! Optional stderr tracing when `-d` / `--debug` is set.

#[derive(Clone, Copy)]
pub struct DebugLog(bool);

impl DebugLog {
    pub fn new(enabled: bool) -> Self {
        Self(enabled)
    }

    pub fn log(self, msg: impl AsRef<str>) {
        if self.0 {
            eprintln!("niri-session(debug): {}", msg.as_ref());
        }
    }
}
