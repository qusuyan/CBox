use mailbox::ClientStub;
use env_logger::fmt::{Color, Style, StyledValue};
use log::Level;
use std::io::Write;

fn colored_level(style: &mut Style, level: Level) -> StyledValue<&'static str> {
    match level {
        Level::Trace => style.set_color(Color::Magenta).value("TRACE"),
        Level::Debug => style.set_color(Color::Blue).value("DEBUG"),
        Level::Info => style.set_color(Color::Green).value("INFO "),
        Level::Warn => style.set_color(Color::Yellow).value("WARN "),
        Level::Error => style.set_color(Color::Red).value("ERROR"),
    }
}

pub fn main() {
    env_logger::builder()
    .format(move |buf, record| {
        let mut style = buf.style();
        let level = colored_level(&mut style, record.level());
        let mut style = buf.style();
        let target = style.set_bold(true).value(record.target());
        writeln!(buf, "{level} {target}: {}", record.args())
    })
    .init();

    let mut stub: ClientStub<String> = ClientStub::new(0).unwrap();
    stub.send(1, "test".to_string()).unwrap();
}