#[derive(Clone, Debug)]
struct Endmark {
    string: &'static str,
    escape: &'static str,
}

const ENDMARK: Endmark = Endmark {
    string: "EOF",
    escape: "\\EOF",
};

pub mod server;
pub(crate) mod message_processing;