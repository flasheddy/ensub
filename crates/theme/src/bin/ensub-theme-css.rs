use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut output = io::stdout().lock();
    output.write_all(ensub_theme::Theme::default().to_css().as_bytes())
}
